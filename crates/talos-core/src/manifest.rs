//! Skill manifests: the declarative half of a skill (docs/SKILLS.md section 1,
//! ADR 0007).
//!
//! The TOML is read into a permissive raw form and then validated into the
//! model below, so a missing language is reported by name instead of as a
//! generic parse error. Any unknown key is rejected, so a typo cannot pass
//! silently.
//!
//! Parsing enforces what the engine needs to run: ids, both languages for
//! summaries, examples and replies, well-formed templates, declared slot
//! types. What only a reviewer needs (names and help lines, reachability of
//! replies, rule overlaps) is checked by [`check`](crate::check), which is
//! what `talos skills check` runs.

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::value::SlotType;
use crate::{Error, IntentId, Lang, render};

/// One value per language. Construction guarantees both are present.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerLang<T> {
    /// The Spanish value.
    pub es: T,
    /// The English value.
    pub en: T,
}

impl<T> PerLang<T> {
    /// The value for a language.
    #[must_use]
    pub fn get(&self, lang: Lang) -> &T {
        match lang {
            Lang::Es => &self.es,
            Lang::En => &self.en,
        }
    }
}

/// A certain-match rule for an intent, in one language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern {
    /// The language whose rule set it joins.
    pub lang: Lang,
    /// A `regex` crate expression, matched against the match form: lower
    /// case, accents removed (see [`normalise`](crate::normalise)). Named
    /// groups (`(?P<name>…)`) capture declared slots.
    pub regex: String,
    /// Slots this pattern sets to fixed values when it matches; each must
    /// be a declared `term` slot.
    pub slots: BTreeMap<String, String>,
}

/// A slot an intent declares (docs/SKILLS.md section 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotDecl {
    /// The name; captures and fixed pattern slots refer to it.
    pub name: String,
    /// What it holds; its extractor turns a capture into a typed value.
    pub kind: SlotType,
    /// The intent cannot be answered without it. Informational in v0.1: a
    /// handler answers a missing required slot with a usage hint; from v0.2
    /// the engine asks back.
    pub required: bool,
    /// A list of terms ("té, café o agua"); only for `term` slots.
    pub repeated: bool,
    /// Steers the handler only: left out of the outcome and of golden
    /// comparison (e.g. which greeting was used).
    pub internal: bool,
}

/// One thing a user can want from a skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Intent {
    /// The fixed identifier, `<skill>.<name>`.
    pub id: IntentId,
    /// Higher wins among rule matches.
    pub priority: i32,
    /// Training examples, per language.
    pub examples: PerLang<Vec<String>>,
    /// Certain-match rules.
    pub patterns: Vec<Pattern>,
    /// The slots it declares, in file order.
    pub slots: Vec<SlotDecl>,
    /// A courtesy (a greeting, a thanks) that may open or close a message
    /// carrying a real request: when the rest of the message is recognised
    /// on its own, this part is dropped (docs/spec/basics.md, "Greeting
    /// prefixes").
    pub courtesy: bool,
}

impl Intent {
    /// The declared slot called `name`.
    #[must_use]
    pub fn slot(&self, name: &str) -> Option<&SlotDecl> {
        self.slots.iter().find(|s| s.name == name)
    }
}

/// A validated skill manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// The skill's identifier.
    pub id: String,
    /// One line for the generated help answer.
    pub summary: PerLang<String>,
    /// What a user calls the skill, for `help.skill`; empty lists when the
    /// manifest gives none (which `talos skills check` reports).
    pub names: PerLang<Vec<String>>,
    /// Example requests shown by help; never training data. Empty when the
    /// manifest gives none (reported by the check).
    pub help: PerLang<Vec<String>>,
    /// Capabilities the skill needs (`network`, `filesystem`, `exec`).
    pub capabilities: Vec<String>,
    /// The skill's intents, in file order.
    pub intents: Vec<Intent>,
    /// Reply templates by id; each has one or more variants per language.
    pub replies: BTreeMap<String, PerLang<Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    skill: RawSkill,
    #[serde(default)]
    intent: Vec<RawIntent>,
    #[serde(default)]
    reply: BTreeMap<String, RawPerLang<Vec<String>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkill {
    id: String,
    #[serde(default)]
    summary: RawPerLang<String>,
    #[serde(default)]
    names: RawPerLang<Vec<String>>,
    #[serde(default)]
    help: RawPerLang<Vec<String>>,
    #[serde(default)]
    capabilities: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawIntent {
    id: String,
    priority: i32,
    #[serde(default)]
    examples: RawPerLang<Vec<String>>,
    #[serde(default)]
    pattern: Vec<RawPattern>,
    #[serde(default)]
    slot: Vec<RawSlot>,
    #[serde(default)]
    courtesy: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlot {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    repeated: bool,
    #[serde(default)]
    internal: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPattern {
    lang: String,
    regex: String,
    #[serde(default)]
    slots: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPerLang<T> {
    es: Option<T>,
    en: Option<T>,
}

impl<T> Default for RawPerLang<T> {
    fn default() -> Self {
        Self { es: None, en: None }
    }
}

/// Something that may be empty, so "present but empty" counts as missing.
trait Blank {
    fn is_blank(&self) -> bool;
}

impl Blank for String {
    fn is_blank(&self) -> bool {
        self.trim().is_empty()
    }
}

impl Blank for Vec<String> {
    fn is_blank(&self) -> bool {
        self.is_empty() || self.iter().any(Blank::is_blank)
    }
}

impl<T: Blank> RawPerLang<T> {
    /// Both languages, or an error naming what lacks which.
    fn both(self, skill: &str, what: &str) -> Result<PerLang<T>, Error> {
        let missing: Vec<&str> = [("es", &self.es), ("en", &self.en)]
            .into_iter()
            .filter(|(_, value)| value.as_ref().is_none_or(Blank::is_blank))
            .map(|(code, _)| code)
            .collect();
        match (self.es, self.en) {
            (Some(es), Some(en)) if missing.is_empty() => Ok(PerLang { es, en }),
            _ => Err(Error::manifest(
                skill,
                format!(
                    "{what} lacks `{}`: both languages are required",
                    missing.join("`, `")
                ),
            )),
        }
    }
}

impl<T: Default> RawPerLang<T> {
    /// Whatever is there; a missing language is left empty for the check to
    /// report.
    fn lenient(self) -> PerLang<T> {
        PerLang {
            es: self.es.unwrap_or_default(),
            en: self.en.unwrap_or_default(),
        }
    }
}

/// The capabilities a skill may declare (DESIGN.md section 11).
pub const CAPABILITIES: [&str; 3] = ["network", "filesystem", "exec"];

/// Parses an intent's slot declarations.
fn slots(skill: &str, intent: &str, raw: Vec<RawSlot>) -> Result<Vec<SlotDecl>, Error> {
    let mut out: Vec<SlotDecl> = Vec::with_capacity(raw.len());
    for slot in raw {
        let at = |message: String| Error::manifest(skill, format!("intent `{intent}`: {message}"));
        if !is_identifier(&slot.name) {
            return Err(at(format!("bad slot name `{}`", slot.name)));
        }
        if out.iter().any(|s| s.name == slot.name) {
            return Err(at(format!("slot `{}` declared twice", slot.name)));
        }
        let kind = SlotType::parse(&slot.kind).ok_or_else(|| {
            at(format!(
                "slot `{}` has unknown type `{}` (expected term, number or date)",
                slot.name, slot.kind
            ))
        })?;
        if slot.repeated && kind != SlotType::Term {
            return Err(at(format!(
                "slot `{}`: only `term` slots may be repeated",
                slot.name
            )));
        }
        out.push(SlotDecl {
            name: slot.name,
            kind,
            required: slot.required,
            repeated: slot.repeated,
            internal: slot.internal,
        });
    }
    Ok(out)
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

impl Manifest {
    /// Parses and validates a manifest. `source` names it in errors (a path
    /// or the skill's directory) until its id is known.
    ///
    /// # Errors
    ///
    /// [`Error::Manifest`] if the TOML is malformed, a key is unknown, an id
    /// is badly formed or repeated, a template is malformed, or any summary,
    /// example list or reply lacks one of the two languages.
    pub fn parse(source: &str, toml_text: &str) -> Result<Self, Error> {
        let raw: RawManifest =
            toml::from_str(toml_text).map_err(|e| Error::manifest(source, e.to_string()))?;
        let id = raw.skill.id;
        if !is_identifier(&id) {
            return Err(Error::manifest(source, format!("bad skill id `{id}`")));
        }
        let summary = raw.skill.summary.both(&id, "skill summary")?;
        if let Some(unknown) = raw
            .skill
            .capabilities
            .iter()
            .find(|c| !CAPABILITIES.contains(&c.as_str()))
        {
            return Err(Error::manifest(
                &id,
                format!("unknown capability `{unknown}`"),
            ));
        }

        let mut seen = BTreeSet::new();
        let mut intents = Vec::with_capacity(raw.intent.len());
        for intent in raw.intent {
            let name = intent
                .id
                .strip_prefix(&id)
                .and_then(|rest| rest.strip_prefix('.'));
            if !name.is_some_and(is_identifier) {
                return Err(Error::manifest(
                    &id,
                    format!("intent `{}` must be `{id}.<name>`", intent.id),
                ));
            }
            if !seen.insert(intent.id.clone()) {
                return Err(Error::manifest(
                    &id,
                    format!("intent `{}` repeated", intent.id),
                ));
            }
            let examples = intent
                .examples
                .both(&id, &format!("intent `{}` examples", intent.id))?;
            let declared = slots(&id, &intent.id, intent.slot)?;
            let patterns = intent
                .pattern
                .into_iter()
                .map(|p| {
                    let at =
                        |m: String| Error::manifest(&id, format!("intent `{}`: {m}", intent.id));
                    let lang = p.lang.parse::<Lang>().map_err(|e| at(e.to_string()))?;
                    for name in p.slots.keys() {
                        match declared.iter().find(|s| &s.name == name) {
                            Some(s) if s.kind == SlotType::Term && !s.repeated => {}
                            Some(_) => {
                                return Err(at(format!(
                                    "a pattern fixes slot `{name}`, which is not a single `term`"
                                )));
                            }
                            None => {
                                return Err(at(format!(
                                    "a pattern fixes slot `{name}`, which is not declared"
                                )));
                            }
                        }
                    }
                    Ok(Pattern {
                        lang,
                        regex: p.regex,
                        slots: p.slots,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            intents.push(Intent {
                id: IntentId(intent.id),
                priority: intent.priority,
                examples,
                patterns,
                slots: declared,
                courtesy: intent.courtesy,
            });
        }

        let mut replies = BTreeMap::new();
        for (reply_id, variants) in raw.reply {
            let variants = variants.both(&id, &format!("reply `{reply_id}`"))?;
            for template in variants.es.iter().chain(&variants.en) {
                render::check(template)
                    .map_err(|e| Error::manifest(&id, format!("reply `{reply_id}`: {e}")))?;
            }
            replies.insert(reply_id, variants);
        }

        Ok(Manifest {
            id,
            summary,
            names: raw.skill.names.lenient(),
            help: raw.skill.help.lenient(),
            capabilities: raw.skill.capabilities,
            intents,
            replies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[skill]
id = "demo"
summary.es = "una prueba"
summary.en = "a test"
names.es = ["prueba"]
names.en = ["test"]

[[intent]]
id = "demo.ping"
priority = 10

  [[intent.slot]]
  name = "kind"
  type = "term"
  internal = true

  [[intent.slot]]
  name = "items"
  type = "term"
  repeated = true

  [intent.examples]
  es = ["ping"]
  en = ["ping"]

  [[intent.pattern]]
  lang = "es"
  regex = '^ping$'
  slots = { kind = "plain" }

[reply.pong]
es = ["Pong.", "¡Pong!"]
en = ["Pong."]
"#;

    fn error(text: &str) -> String {
        Manifest::parse("test", text).unwrap_err().to_string()
    }

    #[test]
    fn parses_a_complete_manifest() {
        let m = Manifest::parse("test", GOOD).unwrap();
        assert_eq!(m.id, "demo");
        assert_eq!(m.summary.get(Lang::En), "a test");
        assert_eq!(m.intents[0].id.as_str(), "demo.ping");
        assert_eq!(m.intents[0].priority, 10);
        assert_eq!(m.intents[0].patterns[0].lang, Lang::Es);
        assert_eq!(m.intents[0].patterns[0].slots["kind"], "plain");
        assert_eq!(m.replies["pong"].get(Lang::Es).len(), 2);
        assert_eq!(m.names.get(Lang::Es), &["prueba"]);
        assert_eq!(m.help.get(Lang::En), &Vec::<String>::new());
        let kind = m.intents[0].slot("kind").unwrap();
        assert!(kind.internal && !kind.required && kind.kind == SlotType::Term);
        assert!(m.intents[0].slot("items").unwrap().repeated);
        assert!(!m.intents[0].courtesy);
    }

    #[test]
    fn rejects_bad_slots() {
        assert!(
            error(&GOOD.replace(
                "type = \"term\"\n  internal",
                "type = \"place\"\n  internal"
            ))
            .contains("unknown type `place`")
        );
        assert!(error(&GOOD.replace("slots = { kind", "slots = { other")).contains("not declared"));
        assert!(error(&GOOD.replace("slots = { kind", "slots = { items")).contains("not a single"));
        assert!(error(&GOOD.replace("name = \"items\"", "name = \"kind\"")).contains("twice"));
        assert!(
            error(&GOOD.replace(
                "type = \"term\"\n  repeated",
                "type = \"number\"\n  repeated"
            ))
            .contains("only `term`")
        );
        assert!(
            error(&GOOD.replace("names.es", "capabilities = [\"teleport\"]\nnames.es"))
                .contains("teleport")
        );
    }

    #[test]
    fn rejects_a_reply_missing_a_language() {
        let text = GOOD.replace("en = [\"Pong.\"]\n", "");
        assert_eq!(
            error(&text),
            "manifest `demo`: reply `pong` lacks `en`: both languages are required"
        );
    }

    #[test]
    fn rejects_an_intent_missing_a_language() {
        let text = GOOD.replace("  es = [\"ping\"]\n", "");
        assert!(error(&text).contains("intent `demo.ping` examples lacks `es`"));
        let text = GOOD.replace("  en = [\"ping\"]\n", "  en = []\n");
        assert!(error(&text).contains("lacks `en`"));
    }

    #[test]
    fn rejects_a_summary_missing_a_language() {
        let text = GOOD.replace("summary.en = \"a test\"\n", "");
        assert!(error(&text).contains("skill summary lacks `en`"));
    }

    #[test]
    fn rejects_unknown_keys_and_bad_ids() {
        assert!(error(&GOOD.replace("priority = 10", "priority = 10\nprio = 3")).contains("prio"));
        assert!(error(&GOOD.replace("demo.ping", "other.ping")).contains("must be `demo.<name>`"));
        assert!(
            error(&GOOD.replace("lang = \"es\"", "lang = \"fr\"")).contains("unknown language")
        );
        assert!(error(&GOOD.replace("Pong.\", \"", "Pong {.\", \"")).contains("reply `pong`"));
        assert!(error(&GOOD.replace("priority = 10\n", "")).contains("priority"));
    }
}
