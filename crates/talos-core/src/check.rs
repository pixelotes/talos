//! The manifest validator behind `talos skills check` (docs/SKILLS.md
//! section 1, ADR 0007).
//!
//! Errors are what would make a skill wrong or unreachable; warnings are
//! what a reviewer should look at. Each finding names its skill.

use std::collections::BTreeSet;
use std::fmt;

use crate::manifest::{Manifest, PerLang};
use crate::normalise::{fold_char, normalise};
use crate::rules::Rules;
use crate::skill::Skill;
use crate::{Lang, render};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// The manifest or handler is wrong; `talos skills check` fails.
    Error,
    /// Worth a look; the check still passes.
    Warning,
}

/// One problem found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// How serious it is.
    pub severity: Severity,
    /// The skill (or manifest source) it concerns.
    pub skill: String,
    /// What is wrong, in one line.
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        write!(f, "{level}: {}: {}", self.skill, self.message)
    }
}

/// Everything the check found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    /// The findings, errors first, then by skill, in the order found.
    pub findings: Vec<Finding>,
    /// How many manifests were read.
    pub skills: usize,
}

impl Report {
    fn push(&mut self, severity: Severity, skill: &str, message: impl Into<String>) {
        self.findings.push(Finding {
            severity,
            skill: skill.to_owned(),
            message: message.into(),
        });
    }

    fn error(&mut self, skill: &str, message: impl Into<String>) {
        self.push(Severity::Error, skill, message);
    }

    fn warning(&mut self, skill: &str, message: impl Into<String>) {
        self.push(Severity::Warning, skill, message);
    }

    /// How many findings have this severity.
    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == severity)
            .count()
    }

    /// Whether anything is an error.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        self.count(Severity::Error) > 0
    }
}

/// Checks a set of skills: manifests as `(source name, TOML text)`, where
/// the source name is the skill's directory, and their handlers.
///
/// Errors: a manifest that does not parse (schema, a missing language, a
/// bad template or slot); a skill id that differs from its directory; a
/// manifest without a handler or a handler without a manifest; missing or
/// one-language `names`/`help`; a reply id a handler declares that the
/// manifest lacks, or a manifest reply no handler declares (unreachable);
/// a pattern that does not compile, captures an undeclared slot, or is
/// written with accents or capitals (it matches the folded form and could
/// never match); two intents of equal priority whose rules in one language
/// both match the same example; a command name declared twice, or a
/// command whose slot its intent does not declare (ADR 0016).
///
/// Warnings: an example matched by a rule of another intent with higher
/// priority; an intent with no pattern (unreachable until the classifier);
/// variants of one reply that use different variables.
#[must_use]
pub fn check(sources: &[(&str, &str)], handlers: &[Box<dyn Skill>]) -> Report {
    let mut report = Report {
        skills: sources.len(),
        ..Report::default()
    };
    let mut manifests = Vec::new();
    for (source, text) in sources {
        match Manifest::parse(source, text) {
            Ok(m) if m.id != *source => {
                report.error(source, format!("id `{}` differs from its directory", m.id));
            }
            Ok(m) => manifests.push(m),
            Err(e) => report.error(source, e.to_string()),
        }
    }
    for handler in handlers {
        if !sources.iter().any(|(source, _)| *source == handler.id()) {
            report.error(handler.id(), "handler has no manifest");
        }
    }
    let mut valid = Vec::new();
    for manifest in manifests {
        let id = manifest.id.clone();
        let errors_before = report.count(Severity::Error);
        match handlers.iter().find(|h| h.id() == id) {
            Some(handler) => replies(&mut report, &manifest, handler.replies()),
            None => report.error(&id, "manifest has no handler"),
        }
        help_fields(&mut report, &manifest);
        patterns(&mut report, &manifest);
        if report.count(Severity::Error) == errors_before {
            valid.push(manifest);
        }
    }
    examples(&mut report, &valid);
    commands(&mut report, &valid);
    report.findings.sort_by_key(|f| f.severity);
    report
}

/// No command name is declared twice, across all skills (ADR 0016). That a
/// command's slot is a public slot of its intent is checked when the
/// manifest is parsed.
fn commands(report: &mut Report, manifests: &[Manifest]) {
    let mut seen: Vec<(&str, &crate::IntentId)> = Vec::new();
    for manifest in manifests {
        for intent in &manifest.intents {
            let Some(command) = &intent.command else {
                continue;
            };
            let names = [command.names.es.as_str(), command.names.en.as_str()];
            for (k, name) in names.iter().enumerate() {
                if k == 1 && names[0] == *name {
                    continue;
                }
                if let Some((_, other)) = seen.iter().find(|(n, _)| n == name) {
                    report.error(
                        &manifest.id,
                        format!(
                            "intent `{}`: command `{name}` is already declared by `{other}`",
                            intent.id
                        ),
                    );
                } else {
                    seen.push((name, &intent.id));
                }
            }
        }
    }
}

/// Reply ids: declared by the handler and present in the manifest, and the
/// other way round; variables consistent across variants.
fn replies(report: &mut Report, manifest: &Manifest, declared: &[&str]) {
    let id = &manifest.id;
    for reply in declared {
        if !manifest.replies.contains_key(*reply) {
            report.error(
                id,
                format!("handler returns reply `{reply}`, which the manifest lacks"),
            );
        }
    }
    for (reply, variants) in &manifest.replies {
        if !declared.contains(&reply.as_str()) {
            report.error(
                id,
                format!("reply `{reply}` is never returned by the handler (unreachable)"),
            );
        }
        let mut sets = Lang::ALL
            .iter()
            .flat_map(|&lang| variants.get(lang))
            .map(|template| render::variables(template));
        if let Some(first) = sets.next()
            && sets.any(|set| set != first)
        {
            report.warning(
                id,
                format!("variants of reply `{reply}` use different variables"),
            );
        }
    }
}

/// `names` and `help` present in both languages (docs/SKILLS.md section 1).
fn help_fields(report: &mut Report, manifest: &Manifest) {
    let fields: [(&str, &PerLang<Vec<String>>); 2] =
        [("names", &manifest.names), ("help", &manifest.help)];
    for (field, values) in fields {
        for lang in Lang::ALL {
            let list = values.get(lang);
            if list.is_empty() || list.iter().any(|v| v.trim().is_empty()) {
                report.error(
                    &manifest.id,
                    format!("`{field}.{lang}` is missing or has a blank entry"),
                );
            }
        }
    }
}

/// The characters of a pattern outside escapes that the match form can
/// never contain: capitals and accented letters.
fn unmatchable(pattern: &str) -> BTreeSet<char> {
    let mut found = BTreeSet::new();
    let mut escaped = false;
    for c in pattern.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c.is_uppercase() || fold_char(c) != c {
            found.insert(c);
        }
    }
    found
}

/// Patterns compile, capture declared slots, are written in the match form;
/// intents have patterns.
fn patterns(report: &mut Report, manifest: &Manifest) {
    let id = &manifest.id;
    if let Err(e) = Rules::compile(std::slice::from_ref(manifest)) {
        report.error(id, e.to_string());
    }
    for intent in &manifest.intents {
        if intent.patterns.is_empty() {
            report.warning(
                id,
                format!(
                    "intent `{}` has no pattern: unreachable until the classifier (v0.2)",
                    intent.id
                ),
            );
        }
        for pattern in &intent.patterns {
            let bad = unmatchable(&pattern.regex);
            if !bad.is_empty() {
                let chars: String = bad.into_iter().collect();
                report.error(
                    id,
                    format!(
                        "intent `{}`: pattern /{}/ uses {chars:?}; patterns match lower-case text \
                         without accents (write `que`, not `qué`)",
                        intent.id, pattern.regex
                    ),
                );
            }
        }
    }
}

/// Every example against every rule of its language: equal-priority
/// conflicts are errors, higher-priority intents taking an example are
/// warnings.
fn examples(report: &mut Report, manifests: &[Manifest]) {
    let Ok(rules) = Rules::compile(manifests) else {
        return;
    };
    let mut conflicts = BTreeSet::new();
    for manifest in manifests {
        for intent in &manifest.intents {
            for lang in Lang::ALL {
                for example in intent.examples.get(lang) {
                    let folded = normalise(example).folded;
                    let hits = rules.all_matches(lang, &folded);
                    for (other, priority, pattern) in &hits {
                        if other != &intent.id && *priority > intent.priority {
                            report.warning(
                                &manifest.id,
                                format!(
                                    "{lang} example {example:?} of `{}` (priority {}) is taken by \
                                     `{other}` (priority {priority}, /{pattern}/)",
                                    intent.id, intent.priority
                                ),
                            );
                        }
                    }
                    for (i, (a, pa, _)) in hits.iter().enumerate() {
                        for (b, pb, _) in &hits[i + 1..] {
                            if a != b
                                && pa == pb
                                && conflicts.insert((lang, a.clone(), b.clone(), folded.clone()))
                            {
                                report.error(
                                    &manifest.id,
                                    format!(
                                        "{lang} example {example:?}: `{a}` and `{b}` both match \
                                         it with priority {pa}"
                                    ),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill::{Call, Context, SkillResult};

    struct Handler(&'static str, &'static [&'static str]);

    impl Skill for Handler {
        fn id(&self) -> &'static str {
            self.0
        }
        fn replies(&self) -> &'static [&'static str] {
            self.1
        }
        fn handle(&self, _: &mut Context<'_>, _: &Call<'_>) -> SkillResult {
            SkillResult::Decline
        }
    }

    const A: &str = r#"
[skill]
id = "a"
summary = { es = "a", en = "a" }
names = { es = ["a"], en = ["a"] }
help = { es = ["hola"], en = ["hello"] }

[[intent]]
id = "a.greet"
priority = 50
examples = { es = ["hola", "buenas"], en = ["hello"] }
pattern = [{ lang = "es", regex = '^hola$' }, { lang = "en", regex = '^hello$' }]

[reply.hi]
es = ["Hola {x}."]
en = ["Hi."]
"#;

    const B: &str = r#"
[skill]
id = "b"
summary = { es = "b", en = "b" }

[[intent]]
id = "b.also"
priority = 50
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "es", regex = '^(hola|buenas)$' }, { lang = "es", regex = '^Qué$' }]

[[intent]]
id = "b.loud"
priority = 90
examples = { es = ["x"], en = ["x"] }
pattern = [{ lang = "en", regex = '^hel+o$' }]

[[intent]]
id = "b.silent"
priority = 10
examples = { es = ["x"], en = ["x"] }

[reply.unused]
es = ["x"]
en = ["x"]
"#;

    fn messages(report: &Report, severity: Severity) -> Vec<String> {
        report
            .findings
            .iter()
            .filter(|f| f.severity == severity)
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn a_clean_skill_passes() {
        let handlers: Vec<Box<dyn Skill>> = vec![Box::new(Handler("a", &["hi"]))];
        let report = check(&[("a", A)], &handlers);
        assert!(!report.has_errors(), "{report:?}");
        assert_eq!(
            messages(&report, Severity::Warning),
            ["warning: a: variants of reply `hi` use different variables"]
        );
    }

    #[test]
    fn finds_what_the_validator_promises() {
        let handlers: Vec<Box<dyn Skill>> = vec![
            Box::new(Handler("a", &["hi", "bye"])),
            Box::new(Handler("b", &[])),
            Box::new(Handler("c", &[])),
        ];
        let report = check(
            &[("a", A), ("b", B), ("x", A), ("d", "[skill]\nid = \"d\"")],
            &handlers,
        );
        let errors = messages(&report, Severity::Error).join("\n");
        for expected in [
            "a: handler returns reply `bye`, which the manifest lacks",
            "b: reply `unused` is never returned by the handler (unreachable)",
            "b: `names.es` is missing",
            "b: `help.en` is missing",
            "/^Qué$/ uses \"Qé\"",
            "x: id `a` differs from its directory",
            "c: handler has no manifest",
            "d: manifest `d`: skill summary lacks",
        ] {
            assert!(
                errors.contains(expected),
                "missing {expected:?} in\n{errors}"
            );
        }
        // b has errors, so its rules are left out of the example checks.
        assert!(!errors.contains("both match"), "{errors}");
        let warnings = messages(&report, Severity::Warning).join("\n");
        assert!(warnings.contains("`b.silent` has no pattern"), "{warnings}");
    }

    #[test]
    fn overlapping_rules_are_reported() {
        let b = B
            .replace(", { lang = \"es\", regex = '^Qué$' }", "")
            .replace(
                "summary = { es = \"b\", en = \"b\" }",
                "summary = { es = \"b\", en = \"b\" }\nnames = { es = [\"b\"], en = [\"b\"] }\n\
                 help = { es = [\"b\"], en = [\"b\"] }",
            );
        let handlers: Vec<Box<dyn Skill>> = vec![
            Box::new(Handler("a", &["hi"])),
            Box::new(Handler("b", &["unused"])),
        ];
        let report = check(&[("a", A), ("b", &b)], &handlers);
        assert_eq!(
            messages(&report, Severity::Error),
            [
                "error: a: es example \"hola\": `a.greet` and `b.also` both match it with priority 50"
            ]
        );
        let warnings = messages(&report, Severity::Warning).join("\n");
        assert!(
            warnings
                .contains("en example \"hello\" of `a.greet` (priority 50) is taken by `b.loud`"),
            "{warnings}"
        );
    }

    #[test]
    fn commands_are_unique_and_fill_declared_slots() {
        let command = |skill: &str, es: &str, slot: &str| {
            A.replace("\"a\"", &format!("\"{skill}\""))
                .replace("a.greet", &format!("{skill}.greet"))
                .replace(
                    "pattern = ",
                    &format!("command = {{ es = \"{es}\", en = \"/hi\"{slot} }}\npattern = "),
                )
        };
        let handlers: Vec<Box<dyn Skill>> = vec![
            Box::new(Handler("a", &["hi"])),
            Box::new(Handler("b", &["hi"])),
        ];
        let (a, b) = (command("a", "/hola", ""), command("b", "/buenas", ""));
        let errors = messages(&check(&[("a", &a), ("b", &b)], &handlers), Severity::Error);
        assert!(
            errors.contains(
                &"error: b: intent `b.greet`: command `/hi` is already declared by `a.greet`"
                    .to_owned()
            ),
            "{errors:?}"
        );
        assert!(!errors.iter().any(|e| e.contains("/hola")), "{errors:?}");
        let b = command("b", "/buenas", ", slot = \"who\"");
        let errors = messages(&check(&[("b", &b)], &handlers[1..]), Severity::Error).join("\n");
        assert!(
            errors.contains("command slot `who` is not a slot"),
            "{errors}"
        );
    }
}
