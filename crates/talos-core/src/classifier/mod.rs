//! Stage 4: the intent classifier (ADR 0008), and step 3 of language
//! detection (ADR 0015), which uses the same features, file format and
//! inference.
//!
//! Character n-grams, words and word pairs of the match form
//! ([`features`]), weighted by TF-IDF and L2-normalised, go into a
//! multinomial linear model trained offline by `tools/train` from the
//! manifests' `[intent.examples]` and the out-of-scope corpus
//! (`reflector/out-of-scope.toml`). The model file is described in
//! `docs/MODEL.md`; inference is a sparse dot product read in place from
//! the file's bytes ([`Model`]).
//!
//! The classifier runs only when no rule matched. Its top class is
//! accepted only if it is an intent (not out-of-scope), its probability is
//! at least the model's threshold (a higher one for a message of very few
//! words), and it leads the runner-up by at least the model's margin. An intent with side effects is never accepted on
//! the classifier's word alone ([`SIDE_EFFECTS`]): its handler may only ask
//! the user to confirm.
//!
//! A model is refused when the hash of the corpus it was trained on is not
//! the hash of the engine's own manifests and out-of-scope corpus: the
//! engine then runs on rules alone and the trace says why.

pub mod features;
pub mod model;
pub mod num;

use std::borrow::Cow;
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::language::Guess;
use crate::manifest::{Manifest, PerLang};
use crate::rules::Candidate;
use crate::{Error, IntentId, Lang};

pub use features::FeatureConfig;
pub use model::{Entry, Kind, Model, ModelData, ModelError, Scored};

/// The class of everything no skill handles. Not an intent id: those always
/// have a dot.
pub const OUT_OF_SCOPE: &str = "out-of-scope";

/// Intents whose handlers change something (DESIGN.md section 11). The
/// classifier never accepts them: they need a rule match or a confirmation.
/// A guess of one that passes the threshold and the margin is offered to
/// its handler only to ask the user ([`Verdict::to_confirm`]), and the
/// engine applies no change made on a guess without a yes. The list is
/// kept here until manifests can say it.
pub const SIDE_EFFECTS: &[&str] = &[
    "teach.assert",
    "teach.forget",
    "teach.phrase",
    "preferences.set",
    "preferences.forget",
];

/// The out-of-scope training corpus, embedded so that the engine can hash
/// the corpus its model must match.
pub const OUT_OF_SCOPE_SOURCE: &str = include_str!("../../../../reflector/out-of-scope.toml");

/// FNV-1a, 64 bits: stable across platforms and releases, unlike `std`'s
/// hasher.
#[derive(Debug, Clone, Copy)]
pub struct Fnv(u64);

impl Fnv {
    /// A fresh hash.
    #[must_use]
    pub const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// Adds bytes.
    pub fn bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    /// Adds a string with its length first, so that concatenations cannot
    /// collide ("ab" + "c" vs "a" + "bc").
    pub fn text(&mut self, text: &str) {
        self.bytes(&(text.len() as u64).to_le_bytes());
        self.bytes(text.as_bytes());
    }

    /// The hash.
    #[must_use]
    pub const fn finish(self) -> u64 {
        self.0
    }
}

impl Default for Fnv {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOutOfScope {
    es: Vec<String>,
    en: Vec<String>,
}

/// Parses the out-of-scope corpus: `es = [...]` and `en = [...]`.
///
/// # Errors
///
/// [`Error::Manifest`] (named `out-of-scope`) if the TOML is malformed or a
/// language is missing.
pub fn parse_out_of_scope(text: &str) -> Result<PerLang<Vec<String>>, Error> {
    let raw: RawOutOfScope =
        toml::from_str(text).map_err(|e| Error::manifest(OUT_OF_SCOPE, e.to_string()))?;
    Ok(PerLang {
        es: raw.es,
        en: raw.en,
    })
}

/// The hash of a training corpus: every intent's examples, by skill id and
/// then in manifest order, Spanish before English, then the out-of-scope
/// examples. Registration order does not change it; neither do comments
/// or formatting.
#[must_use]
pub fn corpus_hash(manifests: &[Manifest], out_of_scope: &PerLang<Vec<String>>) -> u64 {
    let mut hash = Fnv::new();
    let mut sorted: Vec<&Manifest> = manifests.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for manifest in sorted {
        for intent in &manifest.intents {
            hash.text(intent.id.as_str());
            for lang in Lang::ALL {
                let examples = intent.examples.get(lang);
                hash.bytes(&(examples.len() as u64).to_le_bytes());
                for example in examples {
                    hash.text(example);
                }
            }
        }
    }
    hash.text(OUT_OF_SCOPE);
    for lang in Lang::ALL {
        let examples = out_of_scope.get(lang);
        hash.bytes(&(examples.len() as u64).to_le_bytes());
        for example in examples {
            hash.text(example);
        }
    }
    hash.finish()
}

/// The hash of the engine's corpus: these manifests and the embedded
/// out-of-scope corpus.
///
/// # Errors
///
/// The embedded out-of-scope corpus does not parse (a build defect, caught
/// by the tests).
pub fn builtin_corpus_hash(manifests: &[Manifest]) -> Result<u64, Error> {
    Ok(corpus_hash(
        manifests,
        &parse_out_of_scope(OUT_OF_SCOPE_SOURCE)?,
    ))
}

/// Model files as handed to the engine; either may be absent.
#[derive(Debug, Clone, Default)]
pub struct ModelFiles {
    /// The intent model.
    pub intent: Option<Cow<'static, [u8]>>,
    /// The language model.
    pub language: Option<Cow<'static, [u8]>>,
}

/// A loaded model, or why there is none.
type Loaded = Result<Model, String>;

/// The engine's classifiers.
#[derive(Debug, Clone)]
pub struct Classifiers {
    intent: Loaded,
    language: Loaded,
}

impl Default for Classifiers {
    fn default() -> Self {
        Self::none()
    }
}

fn load(bytes: Option<Cow<'static, [u8]>>, kind: Kind, expected: Result<u64, &str>) -> Loaded {
    let bytes = bytes.ok_or_else(|| "no model".to_owned())?;
    let model = Model::open(bytes).map_err(|e| format!("model refused: {e}"))?;
    if model.kind() != kind {
        return Err(format!(
            "model refused: a {} model, not a {kind} one",
            model.kind()
        ));
    }
    let expected = expected.map_err(|e| format!("model refused: {e}"))?;
    if model.corpus_hash() != expected {
        return Err(format!(
            "model refused: trained on corpus {:016x}, the manifests are {expected:016x}; \
             retrain with `cargo run -p talos-train`",
            model.corpus_hash()
        ));
    }
    Ok(model)
}

impl Classifiers {
    /// No models: rules only.
    #[must_use]
    pub fn none() -> Self {
        Self {
            intent: Err("no model".to_owned()),
            language: Err("no model".to_owned()),
        }
    }

    /// Loads the models, refusing any not trained on exactly these manifests
    /// and the embedded out-of-scope corpus.
    #[must_use]
    pub fn load(manifests: &[Manifest], files: ModelFiles) -> Self {
        let expected = builtin_corpus_hash(manifests).map_err(|e| e.to_string());
        let expected = expected.as_ref().copied().map_err(String::as_str);
        Self {
            intent: load(files.intent, Kind::Intent, expected),
            language: load(files.language, Kind::Language, expected),
        }
    }

    /// The intent model, or why there is none.
    ///
    /// # Errors
    ///
    /// Why the model is absent or was refused.
    pub fn intent_model(&self) -> Result<&Model, &str> {
        self.intent.as_ref().map_err(String::as_str)
    }

    /// The language model, or why there is none.
    ///
    /// # Errors
    ///
    /// Why the model is absent or was refused.
    pub fn language_model(&self) -> Result<&Model, &str> {
        self.language.as_ref().map_err(String::as_str)
    }

    /// Step 3 of language detection: the language of a normalised text if
    /// the language model is confident of it. A text with no word of three
    /// letters or more (a sum, a unit, "d20", "?") is left to the
    /// conversation's language: symbols, numbers and short abbreviations
    /// are shared by both languages.
    #[must_use]
    pub fn language(&self, text: &str) -> Option<Guess> {
        let model = self.language.as_ref().ok()?;
        let folded = crate::normalise::fold(text);
        let has_word = features::prepare(&folded)
            .split(' ')
            .any(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 3);
        if !has_word {
            return None;
        }
        let ranked = model.rank(&folded);
        let top = ranked.first()?;
        let lang: Lang = model.classes().get(top.class)?.parse().ok()?;
        (top.probability >= f64::from(model.threshold())).then(|| Guess {
            lang,
            permille: permille(top.probability),
        })
    }

    /// Stage 4: the classifier's verdict on a match-form text.
    #[must_use]
    pub fn recognise(&self, lang: Lang, folded: &str) -> Verdict {
        let model = match &self.intent {
            Ok(model) => model,
            Err(why) => {
                return Verdict {
                    lang,
                    fixed: BTreeMap::new(),
                    ranked: Vec::new(),
                    decision: Decision::Off(why.clone()),
                    threshold: 0.0,
                    margin: 0.0,
                };
            }
        };
        let scored = model.rank(folded);
        let fixed = scored
            .first()
            .and_then(|s| model.class_slots(s.class))
            .cloned()
            .unwrap_or_default();
        let ranked: Vec<(String, f64)> = scored
            .into_iter()
            .filter_map(|s| Some((model.classes().get(s.class)?.clone(), s.probability)))
            .collect();
        let threshold = f64::from(model.threshold_for(features::word_count(folded)));
        let margin = f64::from(model.margin());
        let decision = decide(&ranked, threshold, margin);
        Verdict {
            lang,
            fixed,
            ranked,
            decision,
            threshold,
            margin,
        }
    }
}

fn decide(ranked: &[(String, f64)], threshold: f64, margin: f64) -> Decision {
    let Some((top, p)) = ranked.first() else {
        return Decision::Off("the model has no classes".to_owned());
    };
    let runner_up = ranked.get(1).map_or(0.0, |(_, p)| *p);
    if top == OUT_OF_SCOPE {
        Decision::OutOfScope
    } else if *p < threshold {
        Decision::BelowThreshold
    } else if p - runner_up < margin {
        Decision::NarrowMargin
    } else if SIDE_EFFECTS.contains(&top.as_str()) {
        Decision::SideEffects
    } else {
        Decision::Accepted
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a probability times 1000 is within 0..=1000"
)]
fn permille(p: f64) -> u16 {
    (p.clamp(0.0, 1.0) * 1000.0).round() as u16
}

/// What the classifier decided about one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// No model: rules only. Why.
    Off(String),
    /// The top class is an intent and passed both tests.
    Accepted,
    /// The top class is out-of-scope.
    OutOfScope,
    /// The top class is below the threshold.
    BelowThreshold,
    /// The top class does not lead the runner-up by the margin.
    NarrowMargin,
    /// The top class is an intent with side effects.
    SideEffects,
}

/// The classifier's ranking of one message and its decision.
#[derive(Debug, Clone)]
pub struct Verdict {
    lang: Lang,
    /// The slots the top class carries if accepted.
    fixed: BTreeMap<String, String>,
    /// Every class with its probability, best first.
    ranked: Vec<(String, f64)>,
    decision: Decision,
    threshold: f64,
    margin: f64,
}

impl Verdict {
    /// The decision.
    #[must_use]
    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    /// The classes with their probabilities, best first.
    #[must_use]
    pub fn ranked(&self) -> &[(String, f64)] {
        &self.ranked
    }

    /// The accepted intent with its probability.
    #[must_use]
    pub fn accepted(&self) -> Option<(IntentId, f32)> {
        if self.decision != Decision::Accepted {
            return None;
        }
        let (top, p) = self.ranked.first()?;
        #[allow(clippy::cast_possible_truncation, reason = "a probability")]
        Some((IntentId(top.clone()), *p as f32))
    }

    /// The accepted intent as a candidate for dispatch: no captures, and the
    /// slots its rules fix most often on its examples (`topic = "name"` for
    /// `identity.who`), recorded in the model by the trainer.
    #[must_use]
    pub fn candidates(&self) -> Vec<Candidate> {
        self.accepted()
            .map(|(intent, _)| self.candidate(intent))
            .into_iter()
            .collect()
    }

    /// The top intent when it passed the threshold and the margin but has
    /// side effects ([`Decision::SideEffects`]): a candidate whose handler
    /// must ask the user to confirm before it changes anything (DESIGN.md
    /// section 11). The engine drops any change made on it without a yes.
    #[must_use]
    pub fn to_confirm(&self) -> Vec<Candidate> {
        if self.decision != Decision::SideEffects {
            return Vec::new();
        }
        self.ranked
            .first()
            .map(|(top, _)| self.candidate(IntentId(top.clone())))
            .into_iter()
            .collect()
    }

    /// The probability of the top class, accepted or not.
    #[must_use]
    pub fn top_probability(&self) -> f32 {
        #[allow(clippy::cast_possible_truncation, reason = "a probability")]
        self.ranked.first().map_or(0.0, |(_, p)| *p as f32)
    }

    fn candidate(&self, intent: IntentId) -> Candidate {
        Candidate {
            intent,
            priority: 0,
            lang: self.lang,
            pattern: "classifier".to_owned(),
            fixed: self.fixed.clone(),
            captures: Vec::new(),
            source: crate::rules::Source::Classifier,
        }
    }

    /// The confidence of the accepted intent, 0 when none was.
    #[must_use]
    pub fn confidence(&self) -> f32 {
        self.accepted().map_or(0.0, |(_, p)| p)
    }

    /// The trace's account: the top three classes and why the first was
    /// or was not accepted.
    #[must_use]
    pub fn describe(&self) -> String {
        if let Decision::Off(why) = &self.decision {
            return format!("off: {why}; rules only");
        }
        let top: Vec<String> = self
            .ranked
            .iter()
            .take(3)
            .map(|(class, p)| format!("{class} {p:.3}"))
            .collect();
        let top_line = top.join(", ");
        let (first, p) = self
            .ranked
            .first()
            .map_or(("", 0.0), |(c, p)| (c.as_str(), *p));
        let (second, q) = self
            .ranked
            .get(1)
            .map_or(("", 0.0), |(c, p)| (c.as_str(), *p));
        let why = match self.decision {
            Decision::Off(_) => String::new(),
            Decision::Accepted => format!(
                "accepted {first} ({p:.3} ≥ {:.2}, lead {:.3} ≥ {:.2})",
                self.threshold,
                p - q,
                self.margin
            ),
            Decision::OutOfScope => "not accepted: out-of-scope is the top class".to_owned(),
            Decision::BelowThreshold => format!(
                "not accepted: {first} {p:.3} is below the threshold {:.2}",
                self.threshold
            ),
            Decision::NarrowMargin => format!(
                "not accepted: {first} leads {second} by {:.3}, below the margin {:.2}",
                p - q,
                self.margin
            ),
            Decision::SideEffects => {
                format!("not accepted: {first} has side effects and needs a rule or a confirmation")
            }
        };
        format!("{top_line}; {why}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ranked(list: &[(&str, f64)]) -> Vec<(String, f64)> {
        list.iter().map(|(c, p)| ((*c).to_owned(), *p)).collect()
    }

    #[test]
    fn acceptance_needs_an_intent_the_threshold_and_the_margin() {
        let d = |list: &[(&str, f64)]| decide(&ranked(list), 0.5, 0.2);
        assert_eq!(
            d(&[("clock.now", 0.8), ("clock.date", 0.1)]),
            Decision::Accepted
        );
        assert_eq!(
            d(&[(OUT_OF_SCOPE, 0.9), ("clock.now", 0.05)]),
            Decision::OutOfScope
        );
        assert_eq!(
            d(&[("clock.now", 0.45), ("clock.date", 0.1)]),
            Decision::BelowThreshold
        );
        assert_eq!(
            d(&[("clock.now", 0.55), ("clock.date", 0.4)]),
            Decision::NarrowMargin
        );
        assert_eq!(
            d(&[("teach.assert", 0.95), ("clock.date", 0.01)]),
            Decision::SideEffects
        );
    }

    #[test]
    fn the_out_of_scope_corpus_parses_in_both_languages() {
        let corpus = parse_out_of_scope(OUT_OF_SCOPE_SOURCE).unwrap();
        assert_ne!(corpus.es.len(), 0);
        assert_ne!(corpus.en.len(), 0);
        assert!(parse_out_of_scope("es = []").is_err());
        assert!(parse_out_of_scope("es = []\nen = []\nfr = []").is_err());
    }

    #[test]
    fn the_corpus_hash_ignores_order_of_registration_only() {
        let parse = |id: &str, example: &str| {
            Manifest::parse(
                id,
                &format!(
                    "[skill]\nid = \"{id}\"\nsummary.es = \"x\"\nsummary.en = \"x\"\n\
                     [[intent]]\nid = \"{id}.a\"\npriority = 1\n\
                     examples.es = [\"{example}\"]\nexamples.en = [\"x\"]\n"
                ),
            )
            .unwrap()
        };
        let oos = PerLang {
            es: vec!["a".to_owned()],
            en: vec!["b".to_owned()],
        };
        let a = parse("a", "uno");
        let b = parse("b", "dos");
        let one = corpus_hash(&[a.clone(), b.clone()], &oos);
        assert_eq!(one, corpus_hash(&[b.clone(), a.clone()], &oos));
        assert_ne!(one, corpus_hash(&[a.clone(), parse("b", "tres")], &oos));
        let other = PerLang {
            es: vec!["a".to_owned()],
            en: vec!["c".to_owned()],
        };
        assert_ne!(one, corpus_hash(&[a, b], &other));
    }

    #[test]
    fn a_missing_or_foreign_model_leaves_rules_only() {
        let classifiers = Classifiers::none();
        assert!(classifiers.intent_model().is_err());
        assert!(classifiers.language("hello").is_none());
        let verdict = classifiers.recognise(Lang::Es, "que hora es");
        assert!(matches!(verdict.decision(), Decision::Off(_)));
        assert_eq!(verdict.candidates().len(), 0);
        assert!(verdict.describe().starts_with("off: no model"));

        let data = ModelData {
            kind: Kind::Intent,
            corpus_hash: 1,
            features: FeatureConfig::default(),
            classes: vec![OUT_OF_SCOPE.to_owned()],
            slots: Vec::new(),
            biases: vec![0.0],
            weight_scale: 1.0,
            threshold: 0.5,
            margin: 0.1,
            short_words: 0,
            short_threshold: 0.0,
            vocabulary: Vec::new(),
        };
        let files = ModelFiles {
            intent: Some(Cow::Owned(data.encode().unwrap())),
            language: Some(Cow::Borrowed(b"junk")),
        };
        let classifiers = Classifiers::load(&[], files);
        let why = classifiers.intent_model().unwrap_err();
        assert!(why.contains("trained on corpus 0000000000000001"), "{why}");
        assert!(
            classifiers
                .language_model()
                .unwrap_err()
                .contains("not a Talos model")
        );
    }
}
