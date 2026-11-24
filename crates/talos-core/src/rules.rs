//! Stage 3: rules (DESIGN.md section 4).
//!
//! Every pattern of every manifest is compiled once, at engine construction,
//! into one `RegexSet` per language. A match is certain (confidence 1.0).
//! Candidates are ordered by the manifest's explicit priority, highest first;
//! ties are broken by intent id, then by the pattern's position in its
//! manifest, so the order never depends on registration order.

use regex::RegexSet;

use crate::manifest::Manifest;
use crate::{Error, IntentId, Lang, Slots};

/// A recognised intent, as proposed by a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The intent.
    pub intent: IntentId,
    /// Its priority, from the manifest.
    pub priority: i32,
    /// Slots the matching pattern set.
    pub slots: Slots,
    /// The pattern that matched, for the trace.
    pub pattern: String,
}

/// One compiled pattern's bookkeeping.
#[derive(Debug)]
struct Rule {
    intent: IntentId,
    priority: i32,
    /// Position of the pattern within its manifest, for the tie-break.
    position: usize,
    slots: Slots,
    regex: String,
}

/// The rules of one language.
#[derive(Debug)]
struct LangRules {
    set: RegexSet,
    rules: Vec<Rule>,
}

/// All rules, per language.
#[derive(Debug)]
pub struct Rules {
    by_lang: [LangRules; 2],
}

impl Rules {
    /// Compiles the patterns of every manifest.
    ///
    /// # Errors
    ///
    /// [`Error::Manifest`] naming the skill and intent if a pattern does not
    /// compile.
    pub fn compile(manifests: &[Manifest]) -> Result<Self, Error> {
        let by_lang = Lang::ALL.map(|lang| {
            let mut rules = Vec::new();
            for manifest in manifests {
                let mut position = 0;
                for intent in &manifest.intents {
                    for pattern in &intent.patterns {
                        position += 1;
                        if pattern.lang != lang {
                            continue;
                        }
                        if let Err(e) = regex::Regex::new(&pattern.regex) {
                            return Err(Error::manifest(
                                &manifest.id,
                                format!("intent `{}`: pattern does not compile: {e}", intent.id),
                            ));
                        }
                        rules.push(Rule {
                            intent: intent.id.clone(),
                            priority: intent.priority,
                            position,
                            slots: pattern.slots.clone(),
                            regex: pattern.regex.clone(),
                        });
                    }
                }
            }
            let set = RegexSet::new(rules.iter().map(|r| r.regex.as_str()))
                .map_err(|e| Error::Registry(format!("rule set for `{lang}`: {e}")))?;
            Ok(LangRules { set, rules })
        });
        let [es, en] = by_lang;
        Ok(Self {
            by_lang: [es?, en?],
        })
    }

    /// The intents whose rules in `lang` match `text`, best first, one
    /// candidate per intent.
    #[must_use]
    pub fn candidates(&self, lang: Lang, text: &str) -> Vec<Candidate> {
        let lang_rules = &self.by_lang[lang.index()];
        let mut hits: Vec<&Rule> = lang_rules
            .set
            .matches(text)
            .into_iter()
            .map(|i| &lang_rules.rules[i])
            .collect();
        hits.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.intent.cmp(&b.intent))
                .then_with(|| a.position.cmp(&b.position))
        });
        hits.dedup_by(|later, earlier| later.intent == earlier.intent);
        hits.into_iter()
            .map(|r| Candidate {
                intent: r.intent.clone(),
                priority: r.priority,
                slots: r.slots.clone(),
                pattern: r.regex.clone(),
            })
            .collect()
    }

    /// How many patterns were compiled for a language.
    #[must_use]
    pub fn len(&self, lang: Lang) -> usize {
        self.by_lang[lang.index()].rules.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;

    fn manifest(skill: &str, intents: &[(&str, i32, &str, &str)]) -> Manifest {
        let mut text =
            format!("[skill]\nid = \"{skill}\"\nsummary.es = \"x\"\nsummary.en = \"x\"\n");
        for (name, priority, lang, regex) in intents {
            write!(
                text,
                "[[intent]]\nid = \"{skill}.{name}\"\npriority = {priority}\n\
                 examples.es = [\"x\"]\nexamples.en = [\"x\"]\n\
                 [[intent.pattern]]\nlang = \"{lang}\"\nregex = '{regex}'\n"
            )
            .unwrap();
        }
        Manifest::parse(skill, &text).unwrap()
    }

    fn ids(candidates: &[Candidate]) -> Vec<&str> {
        candidates.iter().map(|c| c.intent.as_str()).collect()
    }

    #[test]
    fn higher_priority_first_then_intent_id() {
        let rules = Rules::compile(&[
            manifest(
                "b",
                &[("low", 10, "es", "^hola"), ("high", 50, "es", "hola")],
            ),
            manifest("a", &[("same", 10, "es", "hola")]),
        ])
        .unwrap();
        assert_eq!(
            ids(&rules.candidates(Lang::Es, "hola")),
            ["b.high", "a.same", "b.low"]
        );
        assert_eq!(rules.candidates(Lang::En, "hola"), []);
        assert_eq!(rules.candidates(Lang::Es, "adiós"), []);
    }

    #[test]
    fn order_does_not_depend_on_registration_order() {
        let a = manifest("a", &[("x", 10, "en", "hi")]);
        let b = manifest("b", &[("x", 10, "en", "hi")]);
        let one = Rules::compile(&[a.clone(), b.clone()]).unwrap();
        let two = Rules::compile(&[b, a]).unwrap();
        assert_eq!(
            one.candidates(Lang::En, "hi"),
            two.candidates(Lang::En, "hi")
        );
        assert_eq!(ids(&one.candidates(Lang::En, "hi")), ["a.x", "b.x"]);
    }

    #[test]
    fn one_candidate_per_intent() {
        let mut m = manifest("a", &[("x", 10, "en", "^hi")]);
        let extra = m.intents[0].patterns[0].clone();
        m.intents[0].patterns.push(crate::manifest::Pattern {
            regex: "hi$".to_owned(),
            ..extra
        });
        let rules = Rules::compile(&[m]).unwrap();
        assert_eq!(rules.len(Lang::En), 2);
        let found = rules.candidates(Lang::En, "hi");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pattern, "^hi");
    }

    #[test]
    fn a_bad_pattern_names_its_skill() {
        let err = Rules::compile(&[manifest("a", &[("x", 10, "es", "(")])]).unwrap_err();
        assert!(err.to_string().starts_with("manifest `a`: intent `a.x`"));
    }
}
