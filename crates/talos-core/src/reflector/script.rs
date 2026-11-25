//! The script format: one TOML file per language under `reflector/` at the
//! workspace root, embedded at build time (docs/spec/reflector.md, ADR 0009).
//!
//! A script holds data only: the ranked rules (sentence shapes and the
//! guards written as patterns), their replies, the replies of the guards
//! decided in code, the distress check's patterns and replies, and the
//! tables the echo is reflected with. The ranking itself is not data: it is
//! [`SHAPES`], fixed by the specification, so a script cannot promote a
//! shape by moving it up the file.
//!
//! ```toml
//! lang = "es"
//! fillers = ["pues", "bueno"]          # dropped from the start of a sentence
//! catch_all = ["No te he entendido.", "…", "…"]
//!
//! [guards]                             # replies of the guards decided in code
//! noise = ["No te he entendido."]
//! other-language = ["Solo hablo español e inglés."]
//! long-text = ["Eso es demasiado texto para mí: …"]
//!
//! [distress]                           # the safety check (S29)
//! patterns = ['quiero morirme']
//! idioms = ['me muero de (?:risa|hambre)']
//! two_numbers = "… llama al {general}; también tienes el {crisis} …"
//! one_number = "… llama al {general} …"
//! no_number = "… llama a tu número local de emergencias …"
//!
//! [[rule]]
//! shape = "opinion"                    # a shape or guard id of SHAPES
//! when = "statement"                   # any (default) | statement | question
//! person = "first"                     # any (default) | first | none
//! patterns = ['^(?P<v>creo que) (?P<x>.+)$']
//! unless = ['^creo que no$']           # skip the rule when one matches
//! replies = ["¿{v} {x}? ¿Por qué lo dices?", "Esa es tu postura."]
//!
//! [reflect]                            # the person tables (reflect.rs)
//! ```
//!
//! Patterns are matched against one sentence of the match form (lower
//! case, no accents, final punctuation dropped). A `{name}` in a reply is
//! the named capture, reflected; a reply with no variable is **neutral**,
//! and is the one used when the echo is blocked.

use std::collections::BTreeMap;
use std::ops::Range;

use serde::Deserialize;

use crate::patterns::Patterns;
use crate::render;
use crate::{Error, Lang, Status};

/// Every rule's shape (or guard written as patterns), highest rank first,
/// with the status it gives (docs/spec/reflector.md 2.1 and 4).
///
/// Two departures from the spec's list, both so that the factual-question
/// guard (`unknown-fact`) can sit below the questions it must not take
/// (4.1 "Not covered"): `why-me` (S13) and `hypothetical` (S9) rank just
/// above it, ahead of the conversational acts.
pub(crate) const SHAPES: &[(&str, Status)] = &[
    ("generative", Status::Unhandled),
    ("advice", Status::Unhandled),
    ("cannot", Status::Unhandled),
    ("not-yet", Status::Unhandled),
    ("insult", Status::Deflected),
    ("compliment", Status::Deflected),
    ("misunderstood", Status::Deflected),
    ("ask-experience", Status::Deflected),
    ("ask-opinion", Status::Deflected),
    ("about-talos", Status::Deflected),
    ("presence", Status::Deflected),
    ("why-me", Status::Deflected),
    ("hypothetical", Status::Deflected),
    ("unknown-fact", Status::Unhandled),
    ("apology", Status::Deflected),
    ("agree", Status::Deflected),
    ("disagree", Status::Deflected),
    ("exclaim", Status::Deflected),
    ("backchannel", Status::Deflected),
    ("compare", Status::Deflected),
    ("taste", Status::Deflected),
    ("plan", Status::Deflected),
    ("wish", Status::Deflected),
    ("complaint", Status::Deflected),
    ("generalisation", Status::Deflected),
    ("opinion", Status::Deflected),
    ("third-party", Status::Deflected),
    ("uncertain", Status::Deflected),
    ("past", Status::Deflected),
    ("self", Status::Deflected),
    ("assertion", Status::Deflected),
];

/// Which sentences a rule applies to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum When {
    /// Questions and statements alike.
    #[default]
    Any,
    /// Only a sentence with no question mark.
    Statement,
    /// Only a sentence opened by `¿` or closed by `?`.
    Question,
}

/// What a rule requires of the persons in the sentence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Person {
    /// Nothing.
    #[default]
    Any,
    /// Some capture speaks in the first person (a plan, a past event).
    First,
    /// The whole sentence has no first- or second-person word (a remark
    /// about the world).
    None,
}

/// A reply template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Template {
    /// The text, with `{name}` for a reflected capture.
    pub(crate) text: String,
    /// The captures it echoes; empty for a neutral reply.
    pub(crate) vars: Vec<String>,
}

impl Template {
    fn parse(text: &str) -> Result<Self, String> {
        render::check(text).map_err(|e| e.to_string())?;
        Ok(Self {
            text: text.to_owned(),
            vars: render::variables(text)
                .into_iter()
                .map(str::to_owned)
                .collect(),
        })
    }

    /// Echoes nothing.
    pub(crate) fn is_neutral(&self) -> bool {
        self.vars.is_empty()
    }
}

/// One rule: a shape (or a guard) recognised by patterns.
#[derive(Debug)]
pub(crate) struct Rule {
    /// The shape id, from [`SHAPES`].
    pub(crate) shape: &'static str,
    /// Its status.
    pub(crate) status: Status,
    /// Its rank: the position of the shape in [`SHAPES`].
    pub(crate) rank: usize,
    pub(crate) when: When,
    pub(crate) person: Person,
    /// Its patterns: these indices of [`Script::patterns`].
    pub(crate) patterns: Range<usize>,
    pub(crate) unless: Patterns,
    pub(crate) replies: Vec<Template>,
}

/// The distress check's data (S29).
#[derive(Debug)]
pub(crate) struct Distress {
    pub(crate) patterns: Patterns,
    pub(crate) idioms: Patterns,
    /// With a general emergency number and a crisis line.
    pub(crate) two_numbers: Template,
    /// With a general emergency number only.
    pub(crate) one_number: Template,
    /// No place known, or no entry for it: no number is named.
    pub(crate) no_number: Template,
}

/// A compiled script.
#[derive(Debug)]
pub(crate) struct Script {
    pub(crate) lang: Lang,
    /// Words dropped from the start of a sentence before matching again.
    pub(crate) fillers: Vec<String>,
    /// The bare catch-all's variants.
    pub(crate) catch_all: Vec<Template>,
    pub(crate) noise: Vec<Template>,
    pub(crate) other_language: Vec<Template>,
    pub(crate) long_text: Vec<Template>,
    pub(crate) distress: Distress,
    /// In rank order; within a shape, in file order.
    pub(crate) rules: Vec<Rule>,
    /// Every pattern of every rule, in rule order.
    pub(crate) patterns: Patterns,
    /// The rule of each pattern in `patterns`.
    pub(crate) pattern_rule: Vec<usize>,
    /// The person tables, as written.
    pub(crate) reflect: RawReflect,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScript {
    lang: String,
    fillers: Vec<String>,
    catch_all: Vec<String>,
    guards: RawGuards,
    distress: RawDistress,
    #[serde(rename = "rule", default)]
    rules: Vec<RawRule>,
    reflect: RawReflect,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawGuards {
    noise: Vec<String>,
    other_language: Vec<String>,
    long_text: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDistress {
    patterns: Vec<String>,
    idioms: Vec<String>,
    two_numbers: String,
    one_number: String,
    no_number: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    shape: String,
    #[serde(default)]
    when: When,
    #[serde(default)]
    person: Person,
    patterns: Vec<String>,
    #[serde(default)]
    unless: Vec<String>,
    replies: Vec<String>,
}

/// The person tables of a script, as written; [`super::reflect`] compiles
/// them. Fields a language does not use stay empty.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawReflect {
    /// First-person word → second-person word ("me" → "te", "my" → "your").
    #[serde(default)]
    pub(crate) words: BTreeMap<String, String>,
    /// Groups of words whose reading the group settles ("creo que" →
    /// "crees que"), tried before single words.
    #[serde(default)]
    pub(crate) phrases: BTreeMap<String, String>,
    /// Words that block the echo: second person, first person plural,
    /// forms with two readings ("siento"), Talos's name.
    #[serde(default)]
    pub(crate) block: Vec<String>,
    /// Groups that block the echo ("let me").
    #[serde(default)]
    pub(crate) block_phrases: Vec<String>,
    /// Fillers removed from a fragment before it is reflected ("you know").
    #[serde(default)]
    pub(crate) drop_phrases: Vec<String>,
    /// Insults and profanity: never echoed.
    #[serde(default)]
    pub(crate) offensive: Vec<String>,
    /// Spanish: words after which a form is a noun ("el", "mi").
    #[serde(default)]
    pub(crate) determiners: Vec<String>,
    /// Spanish: prepositions; a verb-like form after one is unclear.
    #[serde(default)]
    pub(crate) prepositions: Vec<String>,
    /// Spanish: words after which a form is a verb ("yo", "que", "no").
    #[serde(default)]
    pub(crate) verb_context: Vec<String>,
    /// Spanish: auxiliaries; the word after one is a participle.
    #[serde(default)]
    pub(crate) auxiliaries: Vec<String>,
    /// Spanish: nouns and adjectives that look like a verb with a clitic
    /// ("suerte", "fuerte"), copied as written.
    #[serde(default)]
    pub(crate) plain: Vec<String>,
    /// Spanish: conjugation tables.
    #[serde(default)]
    pub(crate) verbs: RawVerbs,
    /// English: the verb after "I" ("am" → "are"); any other form of
    /// "be"/"have"/"do" listed in `bad_after_i` blocks.
    #[serde(default)]
    pub(crate) after_i: BTreeMap<String, String>,
    /// English: forms that are wrong after "I" in the standard ("were"),
    /// so a fragment with them is not echoed.
    #[serde(default)]
    pub(crate) bad_after_i: Vec<String>,
}

/// Spanish conjugation tables. A verb outside them leaves its fragment
/// un-echoed (docs/spec/reflector.md 3.2).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawVerbs {
    /// Regular conjugations by infinitive, with flags after `/`: `ie`,
    /// `ue`, `i` (stem change in the present), `í`, `ú` (stressed stem
    /// vowel: "envío"), `zc` ("conozco"), `y` ("construyo"), `dat` (a verb
    /// like *gustar*, whose subject is the thing: "me gustaría").
    #[serde(default)]
    pub(crate) regular: Vec<String>,
    /// Irregular first-person forms that only the first person has →
    /// the second person ("tengo" → "tienes").
    #[serde(default)]
    pub(crate) irregular: BTreeMap<String, String>,
    /// Irregular forms shared by the first and third persons ("era",
    /// "tuviera") → the second person; reflected only after "yo".
    #[serde(default)]
    pub(crate) shared: BTreeMap<String, String>,
}

fn compile(patterns: &[String]) -> Result<Patterns, String> {
    Patterns::new(patterns.iter().map(String::as_str))
        .map_err(|(i, e)| format!("pattern `{}`: {e}", patterns[i]))
}

fn templates(texts: &[String], what: &str) -> Result<Vec<Template>, String> {
    if texts.is_empty() {
        return Err(format!("{what}: no replies"));
    }
    texts
        .iter()
        .map(|t| Template::parse(t).map_err(|e| format!("{what}: {e}")))
        .collect()
}

fn neutral_only(templates: &[Template], what: &str) -> Result<(), String> {
    match templates.iter().find(|t| !t.is_neutral()) {
        Some(t) => Err(format!("{what}: `{}` echoes, which it may not", t.text)),
        None => Ok(()),
    }
}

impl Script {
    /// Parses and validates a script.
    ///
    /// # Errors
    ///
    /// [`Error::Script`] naming the first problem: bad TOML, an unknown
    /// shape, a pattern that does not compile, a reply that echoes a
    /// capture its patterns do not all have, a deflecting rule with fewer
    /// than three replies or none neutral, a guard reply that echoes.
    pub(crate) fn parse(name: &str, text: &str) -> Result<Self, Error> {
        Self::parse_inner(text).map_err(|message| Error::Script {
            script: name.to_owned(),
            message,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the checks read top to bottom in the order of the file format"
    )]
    fn parse_inner(text: &str) -> Result<Self, String> {
        let raw: RawScript = toml::from_str(text).map_err(|e| e.to_string())?;
        let lang: Lang = raw.lang.parse().map_err(|e| format!("{e}"))?;

        let catch_all = templates(&raw.catch_all, "catch_all")?;
        if catch_all.len() < 3 {
            return Err("catch_all: needs three variants (reflector.md 6)".to_owned());
        }
        let noise = templates(&raw.guards.noise, "guards.noise")?;
        let other_language = templates(&raw.guards.other_language, "guards.other-language")?;
        let long_text = templates(&raw.guards.long_text, "guards.long-text")?;
        for (list, what) in [
            (&catch_all, "catch_all"),
            (&noise, "guards.noise"),
            (&other_language, "guards.other-language"),
            (&long_text, "guards.long-text"),
        ] {
            neutral_only(list, what)?;
        }

        let distress = Distress {
            patterns: compile(&raw.distress.patterns)?,
            idioms: compile(&raw.distress.idioms)?,
            two_numbers: Template::parse(&raw.distress.two_numbers)?,
            one_number: Template::parse(&raw.distress.one_number)?,
            no_number: Template::parse(&raw.distress.no_number)?,
        };
        if distress.patterns.is_empty() {
            return Err("distress: no patterns".to_owned());
        }
        for (template, vars) in [
            (&distress.two_numbers, &["crisis", "general"][..]),
            (&distress.one_number, &["general"][..]),
            (&distress.no_number, &[][..]),
        ] {
            if template.vars != vars {
                return Err(format!(
                    "distress: `{}` must use exactly {vars:?}: the numbers come from \
                     the configuration, the user's words are never echoed",
                    template.text
                ));
            }
        }

        let mut rules = Vec::with_capacity(raw.rules.len());
        for (index, rule) in raw.rules.into_iter().enumerate() {
            let what = format!("rule {} (`{}`)", index + 1, rule.shape);
            let (rank, &(shape, status)) = SHAPES
                .iter()
                .enumerate()
                .find(|(_, (id, _))| *id == rule.shape)
                .ok_or_else(|| format!("{what}: unknown shape"))?;
            if rule.patterns.is_empty() {
                return Err(format!("{what}: no patterns"));
            }
            let patterns = compile(&rule.patterns).map_err(|e| format!("{what}: {e}"))?;
            let unless = compile(&rule.unless).map_err(|e| format!("{what}: {e}"))?;
            let replies = templates(&rule.replies, &what)?;
            for reply in &replies {
                for var in &reply.vars {
                    if let Some(p) = (0..patterns.len()).find(|&p| !patterns.names(p).contains(var))
                    {
                        return Err(format!(
                            "{what}: `{}` uses `{{{var}}}`, which pattern `{}` does not capture",
                            reply.text,
                            patterns.source(p)
                        ));
                    }
                }
            }
            match status {
                Status::Deflected => {
                    if replies.len() < 3 || !replies.iter().any(Template::is_neutral) {
                        return Err(format!(
                            "{what}: a deflection needs at least three replies, one of them \
                             neutral (reflector.md 6)"
                        ));
                    }
                }
                Status::Unhandled | Status::Answered => neutral_only(&replies, &what)?,
            }
            rules.push((
                Rule {
                    shape,
                    status,
                    rank,
                    when: rule.when,
                    person: rule.person,
                    patterns: 0..0,
                    unless,
                    replies,
                },
                patterns,
            ));
        }
        // Stable: within a shape, file order.
        rules.sort_by_key(|(r, _)| r.rank);

        let mut pattern_rule = Vec::new();
        let mut lists = Vec::new();
        let rules = rules
            .into_iter()
            .enumerate()
            .map(|(index, (mut rule, patterns))| {
                let start = pattern_rule.len();
                pattern_rule.extend(std::iter::repeat_n(index, patterns.len()));
                rule.patterns = start..pattern_rule.len();
                lists.push(patterns);
                rule
            })
            .collect();
        let patterns = Patterns::concat(lists);

        Ok(Self {
            lang,
            fillers: raw.fillers,
            catch_all,
            noise,
            other_language,
            long_text,
            distress,
            rules,
            patterns,
            pattern_rule,
            reflect: raw.reflect,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
lang = "es"
fillers = []
catch_all = ["a", "b", "c"]
[guards]
noise = ["n"]
other-language = ["o"]
long-text = ["l"]
[distress]
patterns = ['morir']
idioms = []
two_numbers = "{general} {crisis}"
one_number = "{general}"
no_number = "x"
[reflect]
"#;

    fn with(rules: &str) -> Result<Script, Error> {
        Script::parse("test", &format!("{MINIMAL}\n{rules}"))
    }

    #[test]
    fn a_minimal_script_loads() {
        let script = with("").unwrap();
        assert_eq!(script.lang, Lang::Es);
        assert!(script.rules.is_empty());
    }

    #[test]
    fn rules_are_ranked_by_shape_not_by_file_order() {
        let script = with(
            r#"
[[rule]]
shape = "assertion"
patterns = ['a']
replies = ["x", "y", "z"]
[[rule]]
shape = "insult"
patterns = ['b']
replies = ["x", "y", "z"]
"#,
        )
        .unwrap();
        assert_eq!(script.rules[0].shape, "insult");
        assert_eq!(script.pattern_rule, [0, 1]);
    }

    #[test]
    fn validation_catches_mistakes() {
        let bad = [
            // Unknown shape.
            "[[rule]]\nshape = \"therapy\"\npatterns = ['a']\nreplies = [\"x\"]",
            // Too few variants for a deflection.
            "[[rule]]\nshape = \"opinion\"\npatterns = ['a']\nreplies = [\"x\"]",
            // No neutral variant.
            "[[rule]]\nshape = \"opinion\"\npatterns = ['(?P<x>a)']\nreplies = [\"{x}\", \"{x}.\", \"{x}!\"]",
            // A variable its pattern does not capture.
            "[[rule]]\nshape = \"opinion\"\npatterns = ['a']\nreplies = [\"{x}\", \"b\", \"c\"]",
            // A refusal that echoes.
            "[[rule]]\nshape = \"advice\"\npatterns = ['(?P<x>a)']\nreplies = [\"{x}\"]",
            // A pattern that does not compile.
            "[[rule]]\nshape = \"advice\"\npatterns = ['(']\nreplies = [\"x\"]",
        ];
        for rules in bad {
            assert!(matches!(with(rules), Err(Error::Script { .. })), "{rules}");
        }
        let echoing_distress = MINIMAL.replace("no_number = \"x\"", "no_number = \"{x}\"");
        assert!(Script::parse("test", &echoing_distress).is_err());
    }
}
