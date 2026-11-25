//! Stage 3: rules (DESIGN.md section 4).
//!
//! Every pattern of every manifest is parsed at engine construction into one
//! pattern list per language (`patterns.rs`), which finds the rules that
//! match and reads the winner's captures; its automata are built when first
//! needed. Patterns match the **match form** of the message: lower case,
//! accents folded ([`normalise`](crate::normalise)).
//!
//! A match is certain (confidence 1.0). Candidates are ordered by the
//! manifest's explicit priority, highest first; ties are broken by intent
//! id, then by the pattern's position in its manifest, so the order never
//! depends on registration order.

use std::collections::BTreeMap;
use std::ops::Range;

use crate::manifest::Manifest;
use crate::patterns::Patterns;
use crate::{Error, IntentId, Lang};

/// A recognised intent, as proposed by a rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// The intent.
    pub intent: IntentId,
    /// Its priority, from the manifest.
    pub priority: i32,
    /// The language whose rule set matched.
    pub lang: Lang,
    /// The pattern that matched, for the trace.
    pub pattern: String,
    /// Slots the pattern fixes, by name.
    pub fixed: BTreeMap<String, String>,
    /// Named captures, as byte ranges of the message's match form.
    pub captures: Vec<(String, Range<usize>)>,
    /// What proposed it.
    pub source: Source,
}

/// What proposed a [`Candidate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A manifest pattern: certain.
    Rule,
    /// An entry of the user-correction table: certain.
    Correction,
    /// A command line: certain.
    Command,
    /// The classifier: a guess, with no captures.
    Classifier,
}

/// One pattern's bookkeeping; the pattern has the same index in
/// [`LangRules::patterns`].
#[derive(Debug)]
struct Rule {
    intent: IntentId,
    priority: i32,
    courtesy: bool,
    /// Position of the pattern within its manifest, for the tie-break.
    position: usize,
    fixed: BTreeMap<String, String>,
}

/// The rules of one language.
#[derive(Debug)]
struct LangRules {
    patterns: Patterns,
    rules: Vec<Rule>,
}

/// All rules, per language.
#[derive(Debug)]
pub struct Rules {
    by_lang: [LangRules; 2],
}

/// A courtesy dropped from a message so that the rest could be recognised
/// (docs/spec/basics.md, "Greeting prefixes").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Courtesy {
    /// The intent the dropped part matched, e.g. `smalltalk.greet`.
    pub intent: IntentId,
    /// The dropped part, as a byte range of the match form.
    pub dropped: Range<usize>,
}

/// Characters that separate a courtesy from the request it opens or closes.
const COURTESY_SEPARATORS: &[char] = &[' ', ',', ';', '.', '!'];

/// How many words a courtesy may have.
const COURTESY_WORDS: usize = 4;

impl Rules {
    /// Compiles the patterns of every manifest.
    ///
    /// # Errors
    ///
    /// [`Error::Manifest`] naming the skill and intent if a pattern does not
    /// compile or captures a slot its intent does not declare.
    pub fn compile(manifests: &[Manifest]) -> Result<Self, Error> {
        let by_lang = Lang::ALL.map(|lang| {
            let mut rules = Vec::new();
            let mut sources = Vec::new();
            let mut owners = Vec::new();
            for manifest in manifests {
                let mut position = 0;
                for intent in &manifest.intents {
                    for pattern in &intent.patterns {
                        position += 1;
                        if pattern.lang != lang {
                            continue;
                        }
                        sources.push(pattern.regex.as_str());
                        owners.push((manifest, intent));
                        rules.push(Rule {
                            intent: intent.id.clone(),
                            priority: intent.priority,
                            courtesy: intent.courtesy,
                            position,
                            fixed: pattern.slots.clone(),
                        });
                    }
                }
            }
            let at = |i: usize, message: String| {
                let (manifest, intent) = owners[i];
                Error::manifest(&manifest.id, format!("intent `{}`: {message}", intent.id))
            };
            let patterns = Patterns::new(sources)
                .map_err(|(i, e)| at(i, format!("pattern does not compile: {e}")))?;
            for (i, &(_, intent)) in owners.iter().enumerate() {
                if let Some(name) = patterns
                    .names(i)
                    .iter()
                    .find(|name| intent.slot(name).is_none())
                {
                    return Err(at(
                        i,
                        format!(
                            "pattern /{}/ captures `{name}`, which is not a declared slot",
                            patterns.source(i)
                        ),
                    ));
                }
            }
            Ok(LangRules { patterns, rules })
        });
        let [es, en] = by_lang;
        Ok(Self {
            by_lang: [es?, en?],
        })
    }

    /// Every rule in `lang` that matches `text`, best first, with
    /// repeats of an intent kept.
    fn hits(&self, lang: Lang, text: &str) -> Vec<(usize, &Rule)> {
        let lang_rules = &self.by_lang[lang.index()];
        let mut hits: Vec<(usize, &Rule)> = lang_rules
            .patterns
            .matches(text)
            .into_iter()
            .map(|i| (i, &lang_rules.rules[i]))
            .collect();
        hits.sort_by(|(_, a), (_, b)| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.intent.cmp(&b.intent))
                .then_with(|| a.position.cmp(&b.position))
        });
        hits
    }

    /// The intents whose rules in `lang` match `text`, best first, one
    /// candidate per intent. `text` is (part of) the match form starting at
    /// byte `base`, so captures come back as ranges of the whole.
    #[must_use]
    pub fn candidates(&self, lang: Lang, text: &str, base: usize) -> Vec<Candidate> {
        let patterns = &self.by_lang[lang.index()].patterns;
        let mut hits = self.hits(lang, text);
        hits.dedup_by(|(_, later), (_, earlier)| later.intent == earlier.intent);
        hits.into_iter()
            .map(|(i, r)| {
                let captures = patterns
                    .captures(i, text)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, m)| (name.to_owned(), base + m.start..base + m.end))
                    .collect();
                Candidate {
                    intent: r.intent.clone(),
                    priority: r.priority,
                    lang,
                    pattern: patterns.source(i).to_owned(),
                    fixed: r.fixed.clone(),
                    captures,
                    source: Source::Rule,
                }
            })
            .collect()
    }

    /// Candidates for the whole text: the turn's language first, then the
    /// other (ADR 0011).
    #[must_use]
    pub fn candidates_any(&self, lang: Lang, text: &str, base: usize) -> Vec<Candidate> {
        let found = self.candidates(lang, text, base);
        if found.is_empty() {
            self.candidates(lang.other(), text, base)
        } else {
            found
        }
    }

    /// The courtesy intent `text` is, if a courtesy rule matches it in
    /// either language.
    fn courtesy(&self, lang: Lang, text: &str) -> Option<IntentId> {
        [lang, lang.other()].into_iter().find_map(|l| {
            self.hits(l, text)
                .into_iter()
                .find(|(_, r)| r.courtesy)
                .map(|(_, r)| r.intent.clone())
        })
    }

    /// When the whole message matched nothing: a courtesy (greeting,
    /// thanks) at its start or end is dropped if what is left is
    /// recognised. "hola, qué hora es" is a clock question.
    #[must_use]
    pub fn without_courtesy(&self, lang: Lang, text: &str) -> Option<(Courtesy, Vec<Candidate>)> {
        let boundaries: Vec<usize> = text
            .char_indices()
            .filter(|&(_, c)| c == ' ')
            .map(|(i, _)| i)
            .collect();
        let prefixes = boundaries.iter().take(COURTESY_WORDS);
        let suffixes = boundaries.iter().rev().take(COURTESY_WORDS);
        for (&at, is_prefix) in prefixes
            .map(|b| (b, true))
            .chain(suffixes.map(|b| (b, false)))
        {
            let (head, tail) = text.split_at(at);
            let (courtesy, rest, rest_base) = if is_prefix {
                let rest = tail.trim_start_matches(COURTESY_SEPARATORS);
                (head, rest, text.len() - rest.len())
            } else {
                (tail, head.trim_end_matches(COURTESY_SEPARATORS), 0)
            };
            let courtesy_text = courtesy.trim_matches(COURTESY_SEPARATORS);
            if rest.is_empty() || courtesy_text.is_empty() {
                continue;
            }
            let Some(intent) = self.courtesy(lang, courtesy_text) else {
                continue;
            };
            let found = self.candidates_any(lang, rest, rest_base);
            if !found.is_empty() {
                let dropped = if is_prefix {
                    0..rest_base
                } else {
                    rest.len()..text.len()
                };
                return Some((Courtesy { intent, dropped }, found));
            }
        }
        None
    }

    /// Every rule in `lang` that matches `text` as (intent, priority,
    /// pattern), best first; for `talos skills check`.
    #[must_use]
    pub fn all_matches(&self, lang: Lang, text: &str) -> Vec<(IntentId, i32, String)> {
        let patterns = &self.by_lang[lang.index()].patterns;
        self.hits(lang, text)
            .into_iter()
            .map(|(i, r)| (r.intent.clone(), r.priority, patterns.source(i).to_owned()))
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
                 courtesy = {}\n\
                 examples.es = [\"x\"]\nexamples.en = [\"x\"]\n\
                 slot = [{{ name = \"what\", type = \"term\" }}]\n\
                 [[intent.pattern]]\nlang = \"{lang}\"\nregex = '{regex}'\n",
                *name == "greet"
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
            ids(&rules.candidates(Lang::Es, "hola", 0)),
            ["b.high", "a.same", "b.low"]
        );
        assert_eq!(rules.candidates(Lang::En, "hola", 0), []);
        assert_eq!(rules.candidates(Lang::Es, "adios", 0), []);
        assert_eq!(ids(&rules.candidates_any(Lang::En, "hola", 0)).len(), 3);
    }

    #[test]
    fn order_does_not_depend_on_registration_order() {
        let a = manifest("a", &[("x", 10, "en", "hi")]);
        let b = manifest("b", &[("x", 10, "en", "hi")]);
        let one = Rules::compile(&[a.clone(), b.clone()]).unwrap();
        let two = Rules::compile(&[b, a]).unwrap();
        assert_eq!(
            one.candidates(Lang::En, "hi", 0),
            two.candidates(Lang::En, "hi", 0)
        );
        assert_eq!(ids(&one.candidates(Lang::En, "hi", 0)), ["a.x", "b.x"]);
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
        let found = rules.candidates(Lang::En, "hi", 0);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pattern, "^hi");
        assert_eq!(rules.all_matches(Lang::En, "hi").len(), 2);
    }

    #[test]
    fn captures_are_ranges_of_the_whole_text() {
        let rules =
            Rules::compile(&[manifest("a", &[("x", 10, "es", "^elige (?P<what>.+)$")])]).unwrap();
        let text = "xx elige te o cafe";
        let found = rules.candidates(Lang::Es, &text[3..], 3);
        assert_eq!(found[0].captures, [("what".to_owned(), 9..18)]);
        assert_eq!(&text[9..18], "te o cafe");
    }

    /// Footprint guard (docs/DEVELOPMENT.md, "Footprint"): nothing is
    /// compiled up front, a language's rules only when a turn needs them,
    /// and an ASCII message never needs the costly Unicode form.
    #[test]
    fn rules_are_compiled_when_needed_and_narrow_for_plain_text() {
        let rules = Rules::compile(&[
            manifest("a", &[("x", 10, "es", r"^hola (?P<what>\w+)$")]),
            manifest("b", &[("y", 10, "en", r"^hi \w+$")]),
        ])
        .unwrap();
        let built = |lang: Lang| rules.by_lang[lang.index()].patterns.built();
        assert_eq!([built(Lang::Es), built(Lang::En)], [[false, false]; 2]);
        let found = rules.candidates_any(Lang::Es, "hola mundo", 0);
        assert_eq!(found[0].captures, [("what".to_owned(), 5..10)]);
        assert_eq!(built(Lang::Es), [true, false]);
        assert_eq!(built(Lang::En), [false, false]);
        // A letter beyond ASCII is matched with the pattern as written.
        let found = rules.candidates_any(Lang::Es, "hola σοφία", 0);
        assert_eq!(found[0].captures, [("what".to_owned(), 5..15)]);
        assert_eq!(built(Lang::Es), [true, true]);
    }

    #[test]
    fn bad_patterns_name_their_skill() {
        let err = Rules::compile(&[manifest("a", &[("x", 10, "es", "(")])]).unwrap_err();
        assert!(err.to_string().starts_with("manifest `a`: intent `a.x`"));
        let err = Rules::compile(&[manifest("a", &[("x", 10, "es", "(?P<who>.+)")])]).unwrap_err();
        assert!(
            err.to_string()
                .contains("captures `who`, which is not a declared slot")
        );
    }

    #[test]
    fn a_courtesy_is_dropped_when_the_rest_is_recognised() {
        let rules = Rules::compile(&[manifest(
            "s",
            &[
                ("greet", 100, "es", "^hola$"),
                ("time", 50, "es", "^que hora es\\??$"),
            ],
        )])
        .unwrap();
        let text = "hola, que hora es?";
        assert_eq!(rules.candidates_any(Lang::Es, text, 0), []);
        let (courtesy, found) = rules.without_courtesy(Lang::Es, text).unwrap();
        assert_eq!(courtesy.intent.as_str(), "s.greet");
        assert_eq!(&text[courtesy.dropped], "hola, ");
        assert_eq!(ids(&found), ["s.time"]);
        let (courtesy, found) = rules
            .without_courtesy(Lang::En, "que hora es, hola")
            .unwrap();
        assert_eq!(courtesy.dropped, 11..17);
        assert_eq!(ids(&found), ["s.time"]);
        // Nothing is dropped when the rest means nothing.
        assert!(
            rules
                .without_courtesy(Lang::Es, "hola, me llamo ana")
                .is_none()
        );
        assert!(rules.without_courtesy(Lang::Es, "hola").is_none());
    }
}
