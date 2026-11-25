//! The offline trainer of Talos's classifiers (ADR 0008, ADR 0015;
//! docs/MODEL.md).
//!
//! From the manifests' `[intent.examples]` and the out-of-scope corpus it
//! trains two models with the features of
//! [`talos_core::classifier::features`]:
//!
//! - the **intent** model: one class per intent with examples, plus
//!   `out-of-scope`;
//! - the **language** model: `es` and `en`, from the same phrases.
//!
//! Each is a multinomial logistic regression (softmax) over TF-IDF
//! features, fitted by stochastic gradient descent with L2 regularisation,
//! then pruned and quantised to one signed byte per weight. Everything is
//! deterministic: examples are visited in an order drawn from a fixed seed,
//! arithmetic is `f64` in a fixed order, and `exp`/`ln` are the engine's
//! own ([`talos_core::classifier::num`]), so the same corpus gives the same
//! bytes on every machine.

use std::collections::{BTreeMap, BTreeSet};

use talos_core::Lang;
use talos_core::classifier::{
    Entry, FeatureConfig, Kind, Model, ModelData, OUT_OF_SCOPE, features, num,
};
use talos_core::manifest::{Manifest, PerLang};
use talos_core::normalise::normalise;
use talos_core::random::{RandomSource, SplitMix64};
use talos_core::rules::Rules;

/// One training phrase.
#[derive(Debug, Clone)]
pub struct Example {
    /// The phrase as written.
    pub text: String,
    /// Its match form, as the engine sees it.
    pub folded: String,
    /// Its language.
    pub lang: Lang,
    /// Its intent, or [`OUT_OF_SCOPE`].
    pub class: String,
}

impl Example {
    fn new(text: &str, lang: Lang, class: &str) -> Self {
        Self {
            text: text.to_owned(),
            folded: normalise(text).folded,
            lang,
            class: class.to_owned(),
        }
    }
}

/// Every intent example and every out-of-scope phrase, in a fixed order:
/// skills by id, intents in manifest order, Spanish before English.
#[must_use]
pub fn corpus(manifests: &[Manifest], out_of_scope: &PerLang<Vec<String>>) -> Vec<Example> {
    let mut sorted: Vec<&Manifest> = manifests.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let mut out = Vec::new();
    for manifest in sorted {
        for intent in &manifest.intents {
            for lang in Lang::ALL {
                for text in intent.examples.get(lang) {
                    out.push(Example::new(text, lang, intent.id.as_str()));
                }
            }
        }
    }
    for lang in Lang::ALL {
        for text in out_of_scope.get(lang) {
            out.push(Example::new(text, lang, OUT_OF_SCOPE));
        }
    }
    out
}

/// What training is given besides the examples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Params {
    /// The features.
    pub features: FeatureConfig,
    /// Passes over the examples.
    pub epochs: u32,
    /// L2 regularisation strength.
    pub l2: f64,
    /// Initial learning rate.
    pub rate: f64,
    /// A weight smaller than this fraction of the largest is dropped.
    pub prune: f64,
    /// Recorded in the model: the least probability to accept.
    pub threshold: f32,
    /// Recorded in the model: the least lead over the runner-up.
    pub margin: f32,
    /// Recorded in the model: messages of fewer words need
    /// `short_threshold`.
    pub short_words: u8,
    /// Recorded in the model: the threshold for short messages.
    pub short_threshold: f32,
    /// Seed of the visiting order.
    pub seed: u64,
    /// Each class's examples weigh `(mean class size / class size)` to
    /// this power: 0 leaves the corpus as written, 1 balances classes
    /// fully. The out-of-scope class is much larger than any intent.
    pub balance: f64,
    /// A word seen in fewer phrases than this also counts as an unknown
    /// word in training (0: no unknown-word feature).
    pub rare: u32,
    /// Whether classes learn a bias. Without one, a message with no known
    /// feature is a tie, never a decision.
    pub biases: bool,
}

impl Params {
    /// The parameters of the shipped intent model.
    #[must_use]
    pub fn intent() -> Self {
        Self {
            features: FeatureConfig::default(),
            epochs: 40,
            l2: 3e-6,
            rate: 0.5,
            prune: 0.02,
            threshold: 0.6,
            margin: 0.3,
            short_words: 2,
            short_threshold: 0.85,
            seed: 0x007a_105c_1a55,
            balance: 0.5,
            rare: 2,
            biases: true,
        }
    }

    /// The parameters of the shipped language model.
    #[must_use]
    pub fn language() -> Self {
        Self {
            features: FeatureConfig {
                min_n: 1,
                max_n: 4,
                words: true,
                word_pairs: false,
            },
            threshold: 0.95,
            margin: 0.0,
            short_words: 0,
            short_threshold: 0.0,
            rare: 0,
            biases: false,
            ..Self::intent()
        }
    }
}

/// A trained model before quantisation.
#[derive(Debug, Clone)]
pub struct Trained {
    /// Class names; the index is the class.
    pub classes: Vec<String>,
    /// Features, sorted.
    pub vocabulary: Vec<String>,
    /// Inverse document frequency per feature.
    pub idf: Vec<f64>,
    /// Weights, feature-major: `weights[f * classes + c]`.
    pub weights: Vec<f64>,
    /// One bias per class.
    pub biases: Vec<f64>,
}

/// A phrase's features as a sparse, L2-normalised TF-IDF vector over a
/// vocabulary.
fn vectorise(
    list: &[(String, u32)],
    index: &BTreeMap<String, usize>,
    idf: &[f64],
) -> Vec<(usize, f64)> {
    let mut x: Vec<(usize, f64)> = list
        .iter()
        .filter_map(|(f, count)| index.get(f).map(|&i| (i, f64::from(*count) * idf[i])))
        .collect();
    let norm = x.iter().map(|(_, v)| v * v).sum::<f64>().sqrt();
    if norm > 0.0 {
        for (_, v) in &mut x {
            *v /= norm;
        }
    }
    x
}

/// Adds the unknown-word feature to each phrase's features, as the engine
/// will see it: a word left out of the vocabulary counts as unknown, and
/// so does a word seen in fewer than `params.rare` phrases (it stays a
/// feature too). Rare words stand in for the words the model will never
/// have seen; the out-of-scope class is where most of them are.
fn with_unknown(
    extracted: Vec<Vec<(String, u32)>>,
    df: &BTreeMap<String, u32>,
    keep: Option<&std::collections::BTreeSet<&String>>,
    params: Params,
) -> Vec<Vec<(String, u32)>> {
    extracted
        .into_iter()
        .map(|list| {
            let mut unknown = 0;
            let mut out = Vec::with_capacity(list.len() + 1);
            for (feature, count) in list {
                let kept = keep.is_none_or(|k| k.contains(&feature));
                if features::is_word(&feature) {
                    let rare = df.get(&feature).copied().unwrap_or(0) < params.rare;
                    if !kept || rare {
                        unknown += count;
                    }
                }
                if kept {
                    out.push((feature, count));
                }
            }
            if unknown > 0 && params.rare > 0 {
                out.push((features::UNKNOWN_WORD.to_owned(), unknown));
            }
            out
        })
        .collect()
}

/// How many phrases each feature occurs in.
fn document_frequencies(lists: &[Vec<(String, u32)>]) -> BTreeMap<String, u32> {
    let mut df: BTreeMap<String, u32> = BTreeMap::new();
    for list in lists {
        for (feature, _) in list {
            *df.entry(feature.clone()).or_default() += 1;
        }
    }
    df
}

/// Each class's example weight: `(mean class size / class size)^balance`.
#[allow(clippy::cast_precision_loss, reason = "corpus sizes are small")]
fn class_weights(labels: &[usize], classes: usize, balance: f64) -> Vec<f64> {
    let mut sizes = vec![0usize; classes];
    for &label in labels {
        sizes[label] += 1;
    }
    let present = sizes.iter().filter(|&&size| size > 0).count().max(1);
    let mean = labels.len() as f64 / present as f64;
    sizes
        .iter()
        .map(|&size| {
            if size == 0 {
                0.0
            } else {
                num::exp(balance * num::ln(mean / size as f64))
            }
        })
        .collect()
}

/// Stochastic gradient descent on the softmax cross-entropy with L2
/// regularisation (applied lazily through a common scale). Returns the
/// weights, feature-major, and the biases.
fn descend(
    vectors: &[Vec<(usize, f64)>],
    labels: &[usize],
    features: usize,
    classes: usize,
    params: Params,
) -> (Vec<f64>, Vec<f64>) {
    let weight = class_weights(labels, classes, params.balance);
    let mut raw = vec![0.0f64; features * classes];
    let mut scale = 1.0f64;
    let mut biases = vec![0.0f64; classes];
    let mut rng = SplitMix64::new(params.seed);
    let mut order: Vec<usize> = (0..vectors.len()).collect();
    let mut step = 0.0f64;
    let mut scores = vec![0.0f64; classes];
    for _ in 0..params.epochs {
        // Fisher-Yates with the fixed generator.
        for i in (1..order.len()).rev() {
            #[allow(clippy::cast_possible_truncation, reason = "i + 1 fits")]
            let j = (rng.next_u64() % (i as u64 + 1)) as usize;
            order.swap(i, j);
        }
        for &example in &order {
            let rate = params.rate / (1.0 + params.rate * params.l2 * step);
            step += 1.0;
            let vector = &vectors[example];
            let label = labels[example];
            scores.copy_from_slice(&biases);
            for &(feature, value) in vector {
                for (class, score) in scores.iter_mut().enumerate() {
                    *score += scale * raw[feature * classes + class] * value;
                }
            }
            let top = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let mut total = 0.0;
            for score in &mut scores {
                *score = num::exp(*score - top);
                total += *score;
            }
            // L2 decay of every weight at once, through the scale.
            scale *= 1.0 - rate * params.l2;
            if scale < 1e-9 {
                for w in &mut raw {
                    *w *= scale;
                }
                scale = 1.0;
            }
            for (class, score) in scores.iter().enumerate() {
                let target = if class == label { 1.0 } else { 0.0 };
                let gradient = weight[label] * (score / total - target);
                if params.biases {
                    biases[class] -= rate * gradient;
                }
                for &(feature, value) in vector {
                    raw[feature * classes + class] -= rate * gradient * value / scale;
                }
            }
        }
    }
    (raw.iter().map(|w| w * scale).collect(), biases)
}

/// Fits a model. `classes` fixes the class order; every example's class
/// must be in it. `keep`, when given, restricts the vocabulary.
#[must_use]
pub fn fit(
    examples: &[(&str, usize)],
    classes: &[String],
    params: Params,
    keep: Option<&[String]>,
) -> Trained {
    let extracted: Vec<Vec<(String, u32)>> = examples
        .iter()
        .map(|(folded, _)| features::extract(folded, params.features))
        .collect();
    let raw_df = document_frequencies(&extracted);
    let keep: Option<std::collections::BTreeSet<&String>> = keep.map(|k| k.iter().collect());
    let lists = with_unknown(extracted, &raw_df, keep.as_ref(), params);
    // Document frequencies of what training sees.
    let df = document_frequencies(&lists);
    #[allow(clippy::cast_precision_loss, reason = "corpus sizes are small")]
    let phrases = examples.len() as f64;
    let vocabulary: Vec<String> = df.keys().cloned().collect();
    // The model file stores idf as f32: train with exactly those values.
    let idf: Vec<f64> = df
        .values()
        .map(|&d| {
            f64::from(as_f32(
                num::ln((1.0 + phrases) / (1.0 + f64::from(d))) + 1.0,
            ))
        })
        .collect();
    let index: BTreeMap<String, usize> = vocabulary
        .iter()
        .enumerate()
        .map(|(i, f)| (f.clone(), i))
        .collect();
    let vectors: Vec<Vec<(usize, f64)>> = lists
        .iter()
        .map(|list| vectorise(list, &index, &idf))
        .collect();
    let labels: Vec<usize> = examples.iter().map(|(_, label)| *label).collect();
    let (weights, biases) = descend(&vectors, &labels, vocabulary.len(), classes.len(), params);
    Trained {
        classes: classes.to_vec(),
        vocabulary,
        idf,
        weights,
        biases,
    }
}

#[allow(clippy::cast_possible_truncation, reason = "the model stores f32")]
fn as_f32(v: f64) -> f32 {
    v as f32
}

impl Trained {
    /// The largest weight in absolute value.
    fn largest(&self) -> f64 {
        self.weights.iter().fold(0.0, |m, w| f64::max(m, w.abs()))
    }

    /// Features with at least one weight that survives pruning.
    #[must_use]
    pub fn useful_features(&self, prune: f64) -> Vec<String> {
        let floor = prune * self.largest();
        let c = self.classes.len();
        self.vocabulary
            .iter()
            .enumerate()
            .filter(|(f, _)| {
                self.weights[f * c..(f + 1) * c]
                    .iter()
                    .any(|w| w.abs() >= floor)
            })
            .map(|(_, name)| name.clone())
            .collect()
    }

    /// Prunes and quantises into a model file's contents.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "quantised weights are clamped to ±127; classes are fewer than 256"
    )]
    pub fn quantise(&self, kind: Kind, corpus_hash: u64, params: Params) -> ModelData {
        let largest = self.largest().max(f64::MIN_POSITIVE);
        let step = largest / 127.0;
        let floor = params.prune * largest;
        let c = self.classes.len();
        let vocabulary = self
            .vocabulary
            .iter()
            .enumerate()
            .filter_map(|(f, feature)| {
                let weights: Vec<(u8, i8)> = (0..c)
                    .filter_map(|k| {
                        let w = self.weights[f * c + k];
                        if w.abs() < floor {
                            return None;
                        }
                        let q = (w / step).round().clamp(-127.0, 127.0) as i8;
                        (q != 0).then_some((k as u8, q))
                    })
                    .collect();
                (!weights.is_empty()).then(|| Entry {
                    feature: feature.clone(),
                    idf: as_f32(self.idf[f]),
                    weights,
                })
            })
            .collect();
        ModelData {
            kind,
            corpus_hash,
            features: params.features,
            classes: self.classes.clone(),
            slots: Vec::new(),
            biases: self.biases.iter().map(|&b| as_f32(b)).collect(),
            weight_scale: as_f32(step),
            threshold: params.threshold,
            margin: params.margin,
            short_words: params.short_words,
            short_threshold: params.short_threshold,
            vocabulary,
        }
    }
}

/// Trains a model end to end: a first fit chooses the features whose
/// weights survive pruning, a second fit on those alone gives the weights
/// (so the engine's normalisation, which sees only kept features, matches
/// training), then pruning and quantisation.
#[must_use]
pub fn train(
    examples: &[(&str, usize)],
    classes: &[String],
    kind: Kind,
    corpus_hash: u64,
    params: Params,
) -> ModelData {
    let first = fit(examples, classes, params, None);
    let keep = first.useful_features(params.prune);
    let second = fit(examples, classes, params, Some(&keep));
    second.quantise(kind, corpus_hash, params)
}

/// The intent model's classes: every intent with examples, sorted, then
/// out-of-scope.
#[must_use]
pub fn intent_classes(corpus: &[Example]) -> Vec<String> {
    let mut classes: Vec<String> = corpus
        .iter()
        .filter(|e| e.class != OUT_OF_SCOPE)
        .map(|e| e.class.clone())
        .collect();
    classes.sort();
    classes.dedup();
    classes.push(OUT_OF_SCOPE.to_owned());
    classes
}

/// The language model's classes.
#[must_use]
pub fn language_classes() -> Vec<String> {
    Lang::ALL.iter().map(|l| l.code().to_owned()).collect()
}

fn labelled<'a>(
    corpus: &'a [Example],
    classes: &[String],
    language: bool,
) -> Vec<(&'a str, usize)> {
    corpus
        .iter()
        .filter_map(|e| {
            let label = if language {
                e.lang.code()
            } else {
                e.class.as_str()
            };
            classes
                .iter()
                .position(|c| c == label)
                .map(|i| (e.folded.as_str(), i))
        })
        .collect()
}

/// The slot names every pattern of each intent fixes: a handler may rely
/// on them, so a guess of the intent must carry them too.
fn always_fixed(manifests: &[Manifest]) -> BTreeMap<String, BTreeSet<String>> {
    let mut out = BTreeMap::new();
    for manifest in manifests {
        for intent in &manifest.intents {
            let mut patterns = intent.patterns.iter();
            let Some(first) = patterns.next() else {
                continue;
            };
            let mut keys: BTreeSet<String> = first.slots.keys().cloned().collect();
            for pattern in patterns {
                keys.retain(|k| pattern.slots.contains_key(k));
            }
            if !keys.is_empty() {
                out.insert(intent.id.as_str().to_owned(), keys);
            }
        }
    }
    out
}

/// Per intent class, the slots an accepted guess carries: for each slot
/// that every pattern of the intent fixes (`identity`'s `topic`), the value
/// its rules give most often on the intent's own examples (ties to the
/// smallest). Most intents carry none.
#[must_use]
pub fn class_slots(
    manifests: &[Manifest],
    corpus: &[Example],
    classes: &[String],
) -> Vec<BTreeMap<String, String>> {
    let Ok(rules) = Rules::compile(manifests) else {
        return vec![BTreeMap::new(); classes.len()];
    };
    let needed = always_fixed(manifests);
    // (intent, slot) -> value -> count.
    let mut counts: BTreeMap<(&str, String), BTreeMap<String, usize>> = BTreeMap::new();
    for example in corpus {
        let Some(keys) = needed.get(&example.class) else {
            continue;
        };
        let own = rules
            .candidates_any(example.lang, &example.folded, 0)
            .into_iter()
            .find(|c| c.intent.as_str() == example.class);
        if let Some(candidate) = own {
            for (key, value) in candidate
                .fixed
                .into_iter()
                .filter(|(k, _)| keys.contains(k))
            {
                *counts
                    .entry((example.class.as_str(), key))
                    .or_default()
                    .entry(value)
                    .or_default() += 1;
            }
        }
    }
    classes
        .iter()
        .map(|class| {
            counts
                .iter()
                .filter(|((intent, _), _)| intent == class)
                .filter_map(|((_, key), values)| {
                    // Most frequent; among equals the first (smallest) value.
                    let (value, _) = values.iter().fold(
                        None::<(&String, usize)>,
                        |best, (value, n)| match best {
                            Some((_, m)) if m >= *n => best,
                            _ => Some((value, *n)),
                        },
                    )?;
                    Some((key.clone(), value.clone()))
                })
                .collect()
        })
        .collect()
}

/// Trains the intent model.
#[must_use]
pub fn train_intent(corpus: &[Example], corpus_hash: u64, params: Params) -> ModelData {
    let classes = intent_classes(corpus);
    train(
        &labelled(corpus, &classes, false),
        &classes,
        Kind::Intent,
        corpus_hash,
        params,
    )
}

/// Trains the language model.
#[must_use]
pub fn train_language(corpus: &[Example], corpus_hash: u64, params: Params) -> ModelData {
    let classes = language_classes();
    train(
        &labelled(corpus, &classes, true),
        &classes,
        Kind::Language,
        corpus_hash,
        params,
    )
}

/// Results of cross-validation, per language.
#[derive(Debug, Default, Clone, Copy)]
pub struct Fold {
    /// Intent examples seen.
    pub intents: usize,
    /// Of those, accepted as their own intent.
    pub correct: usize,
    /// Accepted as another intent.
    pub confused: usize,
    /// Out-of-scope examples seen.
    pub out_of_scope: usize,
    /// Of those, accepted as an intent.
    pub false_accepts: usize,
    /// Language model: examples and correct confident guesses, and wrong
    /// confident guesses.
    pub lang_rows: usize,
    /// Confident and right.
    pub lang_right: usize,
    /// Confident and wrong.
    pub lang_wrong: usize,
}

/// A training phrase cross-validation got wrong: (phrase, its class, what
/// the held-out model accepted, or `None`).
pub type Miss = (String, String, Option<String>);

/// K-fold cross-validation of both models on the training corpus itself:
/// a guide for choosing parameters that never looks at the batteries.
#[must_use]
pub fn cross_validate(
    corpus: &[Example],
    k: usize,
    intent: Params,
    language: Params,
) -> (PerLang<Fold>, Vec<Miss>) {
    let mut misses = Vec::new();
    let k = k.max(2);
    let mut rng = SplitMix64::new(0x0c0f_fee0);
    let mut folds: Vec<usize> = (0..corpus.len()).map(|i| i % k).collect();
    for i in (1..folds.len()).rev() {
        #[allow(clippy::cast_possible_truncation, reason = "i + 1 fits")]
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        folds.swap(i, j);
    }
    let mut result = PerLang {
        es: Fold::default(),
        en: Fold::default(),
    };
    for fold in 0..k {
        let train_set: Vec<Example> = corpus
            .iter()
            .zip(&folds)
            .filter(|(_, f)| **f != fold)
            .map(|(e, _)| e.clone())
            .collect();
        let intent_model = Model::open(
            train_intent(&train_set, 0, intent)
                .encode()
                .unwrap_or_default(),
        );
        let language_model = Model::open(
            train_language(&train_set, 0, language)
                .encode()
                .unwrap_or_default(),
        );
        let (Ok(intent_model), Ok(language_model)) = (intent_model, language_model) else {
            continue;
        };
        for (example, _) in corpus.iter().zip(&folds).filter(|(_, f)| **f == fold) {
            let r = match example.lang {
                Lang::Es => &mut result.es,
                Lang::En => &mut result.en,
            };
            let accepted = accept(&intent_model, &example.folded);
            let right = if example.class == OUT_OF_SCOPE {
                r.out_of_scope += 1;
                r.false_accepts += usize::from(accepted.is_some());
                accepted.is_none()
            } else {
                r.intents += 1;
                match &accepted {
                    Some(class) if *class == example.class => r.correct += 1,
                    Some(_) => r.confused += 1,
                    None => {}
                }
                accepted.as_deref() == Some(example.class.as_str())
            };
            if !right {
                misses.push((example.text.clone(), example.class.clone(), accepted));
            }
            r.lang_rows += 1;
            let ranked = language_model.rank(&example.folded);
            if let Some(top) = ranked.first()
                && top.probability >= f64::from(language_model.threshold())
            {
                if language_model.classes()[top.class] == example.lang.code() {
                    r.lang_right += 1;
                } else {
                    r.lang_wrong += 1;
                }
            }
        }
    }
    (result, misses)
}

/// The intent a model accepts for a phrase, by the engine's rules.
#[must_use]
pub fn accept(model: &Model, folded: &str) -> Option<String> {
    let ranked = model.rank(folded);
    let top = ranked.first()?;
    let runner_up = ranked.get(1).map_or(0.0, |s| s.probability);
    let class = model.classes().get(top.class)?;
    let threshold = model.threshold_for(features::word_count(folded));
    (class != OUT_OF_SCOPE
        && top.probability >= f64::from(threshold)
        && top.probability - runner_up >= f64::from(model.margin()))
    .then(|| class.clone())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "tests")]
    use super::*;

    fn tiny() -> Vec<Example> {
        let mut out = Vec::new();
        for (text, lang, class) in [
            ("que hora es", Lang::Es, "clock.now"),
            ("dime la hora", Lang::Es, "clock.now"),
            ("what time is it", Lang::En, "clock.now"),
            ("tell me the time", Lang::En, "clock.now"),
            ("tira un dado", Lang::Es, "random.dice"),
            ("lanza los dados", Lang::Es, "random.dice"),
            ("roll a die", Lang::En, "random.dice"),
            ("roll the dice", Lang::En, "random.dice"),
            ("escribe un poema", Lang::Es, OUT_OF_SCOPE),
            ("write a poem", Lang::En, OUT_OF_SCOPE),
        ] {
            out.push(Example::new(text, lang, class));
        }
        out
    }

    #[test]
    fn training_is_deterministic_and_learns_the_examples() {
        let corpus = tiny();
        let a = train_intent(&corpus, 7, Params::intent()).encode().unwrap();
        let b = train_intent(&corpus, 7, Params::intent()).encode().unwrap();
        assert_eq!(a, b);
        let model = Model::open(a).unwrap();
        model.verify().unwrap();
        assert_eq!(model.corpus_hash(), 7);
        assert_eq!(accept(&model, "que hora es").as_deref(), Some("clock.now"));
        assert_eq!(
            accept(&model, "roll the dice").as_deref(),
            Some("random.dice")
        );
        assert_eq!(accept(&model, "write a poem"), None);
        let language = Model::open(
            train_language(&corpus, 7, Params::language())
                .encode()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(language.classes(), ["es", "en"]);
    }
}
