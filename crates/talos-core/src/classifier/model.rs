//! The model file (docs/MODEL.md): writing it ([`ModelData::encode`], for
//! the trainer) and reading it in place ([`Model`]).
//!
//! A [`Model`] keeps the file's bytes and reads every value from them when
//! it is needed: nothing is copied into the heap. Embedded in the binary,
//! the file is therefore paged in by the operating system only where a
//! turn touches it, like a memory-mapped file. Opening checks the header
//! and that the sections fit the file; every later read is bounds-checked,
//! so a damaged file can give a wrong ranking but never a panic.
//! [`Model::verify`] checks everything else (the checksum, the order of
//! the vocabulary, the class indices); tests and `talos skills check` run
//! it, the engine does not, so that start-up reads one page.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use super::features::{FeatureConfig, UNKNOWN_WORD, extract, is_word};
use super::{Fnv, num};

/// The file's first eight bytes.
pub const MAGIC: [u8; 8] = *b"TALOSCLF";
/// The format version this build reads and writes.
pub const VERSION: u16 = 1;
/// Bytes before the class table.
const HEADER: usize = 56;

/// What a model classifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Intents, plus the out-of-scope class (ADR 0008).
    Intent,
    /// Spanish or English (ADR 0015, step 3).
    Language,
}

impl Kind {
    const fn code(self) -> u8 {
        match self {
            Kind::Intent => 1,
            Kind::Language => 2,
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Kind::Intent),
            2 => Some(Kind::Language),
            _ => None,
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Intent => "intent",
            Kind::Language => "language",
        })
    }
}

/// Why a model file was not accepted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelError(pub String);

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ModelError {}

fn bad(what: impl Into<String>) -> ModelError {
    ModelError(what.into())
}

/// One vocabulary entry of a model being written.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The feature, as [`extract`] spells it.
    pub feature: String,
    /// Its inverse document frequency.
    pub idf: f32,
    /// Its quantised weights: (class index, weight in units of the
    /// model's `weight_scale`). Zero weights are left out.
    pub weights: Vec<(u8, i8)>,
}

/// A model as the trainer builds it, before encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelData {
    /// What it classifies.
    pub kind: Kind,
    /// The hash of the corpus it was trained on ([`corpus_hash`](super::corpus_hash)).
    pub corpus_hash: u64,
    /// The features it reads.
    pub features: FeatureConfig,
    /// Class names, at most 255.
    pub classes: Vec<String>,
    /// Per class, the slots an accepted guess carries as if a rule had
    /// fixed them (`topic = "name"`): for an intent, those its rules fix
    /// most often on its examples. Missing entries are empty.
    pub slots: Vec<BTreeMap<String, String>>,
    /// One bias per class.
    pub biases: Vec<f32>,
    /// A weight is its quantised value times this.
    pub weight_scale: f32,
    /// The least probability a top class needs to be accepted.
    pub threshold: f32,
    /// The least lead in probability the top class needs over the
    /// runner-up.
    pub margin: f32,
    /// A message of fewer words than this needs `short_threshold`
    /// instead of `threshold` (0: no such rule). Short messages carry
    /// little evidence, and the common ones are matched by rules anyway.
    pub short_words: u8,
    /// The threshold for short messages.
    pub short_threshold: f32,
    /// The vocabulary; [`ModelData::encode`] sorts it.
    pub vocabulary: Vec<Entry>,
}

fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, v: f32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// A string of at most 255 bytes, after its length byte.
fn put_short(out: &mut Vec<u8>, text: &str) -> Result<(), ModelError> {
    let len = u8::try_from(text.len()).map_err(|_| bad(format!("`{text}` is too long")))?;
    out.push(len);
    out.extend_from_slice(text.as_bytes());
    Ok(())
}

/// The string written by [`put_short`] at `at`, and the offset after it.
fn short_at(b: &[u8], at: usize) -> Result<(String, usize), ModelError> {
    let len = usize::from(*b.get(at).ok_or_else(|| bad("truncated class table"))?);
    let text = b
        .get(at + 1..at + 1 + len)
        .and_then(|n| std::str::from_utf8(n).ok())
        .ok_or_else(|| bad("malformed class table"))?;
    Ok((text.to_owned(), at + 1 + len))
}

fn len_u32(n: usize, what: &str) -> Result<u32, ModelError> {
    u32::try_from(n).map_err(|_| bad(format!("too many {what}")))
}

impl ModelData {
    /// The file's bytes.
    ///
    /// # Errors
    ///
    /// More than 255 classes, a class name, slot name or slot value longer
    /// than 255 bytes, more than 255 slots for a class, a bias list of the
    /// wrong length, a weight naming no class, or a section larger than
    /// 4 GiB.
    pub fn encode(&self) -> Result<Vec<u8>, ModelError> {
        let classes = u8::try_from(self.classes.len())
            .map_err(|_| bad("more than 255 classes"))
            .and_then(|n| {
                if n == 0 {
                    Err(bad("no classes"))
                } else {
                    Ok(n)
                }
            })?;
        if self.biases.len() != self.classes.len() {
            return Err(bad("one bias per class is needed"));
        }
        let mut vocabulary: Vec<&Entry> = self.vocabulary.iter().collect();
        vocabulary.sort_by(|a, b| a.feature.as_bytes().cmp(b.feature.as_bytes()));
        vocabulary.dedup_by(|a, b| a.feature == b.feature);
        let features = len_u32(vocabulary.len(), "features")?;
        let postings: usize = vocabulary.iter().map(|e| e.weights.len()).sum();
        let postings = len_u32(postings, "weights")?;

        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        put_u16(&mut out, VERSION);
        out.push(self.kind.code());
        out.push(classes);
        out.extend_from_slice(&self.corpus_hash.to_le_bytes());
        out.push(self.features.min_n);
        out.push(self.features.max_n);
        out.push(u8::from(self.features.words));
        out.push(u8::from(self.features.word_pairs));
        put_u32(&mut out, features);
        put_u32(&mut out, postings);
        put_f32(&mut out, self.weight_scale);
        put_f32(&mut out, self.threshold);
        put_f32(&mut out, self.margin);
        put_f32(&mut out, self.short_threshold);
        out.push(self.short_words);
        // Reserved, zero.
        out.extend_from_slice(&[0; 7]);
        debug_assert_eq!(out.len(), HEADER);

        let none = BTreeMap::new();
        for (i, name) in self.classes.iter().enumerate() {
            put_short(&mut out, name)?;
            let slots = self.slots.get(i).unwrap_or(&none);
            out.push(u8::try_from(slots.len()).map_err(|_| bad("too many slots"))?);
            for (key, value) in slots {
                put_short(&mut out, key)?;
                put_short(&mut out, value)?;
            }
        }
        for &bias in &self.biases {
            put_f32(&mut out, bias);
        }
        let mut at = 0u32;
        for entry in &vocabulary {
            put_u32(&mut out, at);
            at = at
                .checked_add(len_u32(entry.feature.len(), "vocabulary bytes")?)
                .ok_or_else(|| bad("vocabulary too large"))?;
        }
        put_u32(&mut out, at);
        for entry in &vocabulary {
            out.extend_from_slice(entry.feature.as_bytes());
        }
        for entry in &vocabulary {
            put_f32(&mut out, entry.idf);
        }
        let mut at = 0u32;
        for entry in &vocabulary {
            put_u32(&mut out, at);
            at += len_u32(entry.weights.len(), "weights")?;
        }
        put_u32(&mut out, at);
        for entry in &vocabulary {
            let mut weights = entry.weights.clone();
            weights.sort_unstable();
            for (class, weight) in weights {
                if class >= classes {
                    return Err(bad(format!(
                        "weight for class {class}, which does not exist"
                    )));
                }
                out.push(class);
                out.extend_from_slice(&weight.to_le_bytes());
            }
        }
        let mut hash = Fnv::new();
        hash.bytes(&out);
        out.extend_from_slice(&hash.finish().to_le_bytes());
        Ok(out)
    }
}

/// A class with its probability.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    /// Index into [`Model::classes`].
    pub class: usize,
    /// Softmax probability.
    pub probability: f64,
}

/// A model file, read in place.
#[derive(Debug, Clone)]
pub struct Model {
    bytes: Cow<'static, [u8]>,
    kind: Kind,
    corpus_hash: u64,
    features: FeatureConfig,
    weight_scale: f32,
    threshold: f32,
    margin: f32,
    short_words: u8,
    short_threshold: f32,
    classes: Vec<String>,
    slots: Vec<BTreeMap<String, String>>,
    /// Number of features.
    count: usize,
    biases: usize,
    vocab_offsets: usize,
    vocab_bytes: usize,
    idf: usize,
    post_offsets: usize,
    postings: usize,
    end: usize,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

fn f32_at(bytes: &[u8], at: usize) -> Option<f32> {
    Some(f32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn finite(value: Option<f32>, what: &str) -> Result<f32, ModelError> {
    value
        .filter(|v| v.is_finite())
        .ok_or_else(|| bad(format!("{what} is missing or not a number")))
}

impl Model {
    /// Opens a model file: checks the header and that every section fits.
    ///
    /// # Errors
    ///
    /// Not a model file, another format version, or a file whose sections
    /// do not fit its length.
    pub fn open(bytes: impl Into<Cow<'static, [u8]>>) -> Result<Self, ModelError> {
        let bytes = bytes.into();
        let b: &[u8] = &bytes;
        if b.get(..8) != Some(&MAGIC[..]) {
            return Err(bad("not a Talos model file"));
        }
        let version = u16_at(b, 8).ok_or_else(|| bad("truncated header"))?;
        if version != VERSION {
            return Err(bad(format!(
                "model format version {version}; this build reads version {VERSION}"
            )));
        }
        if b.len() < HEADER {
            return Err(bad("truncated header"));
        }
        let kind = Kind::from_code(b[10]).ok_or_else(|| bad("unknown model kind"))?;
        let class_count = usize::from(b[11]);
        let corpus_hash = u64_at(b, 12).ok_or_else(|| bad("truncated header"))?;
        let features = FeatureConfig {
            min_n: b[20],
            max_n: b[21],
            words: b[22] != 0,
            word_pairs: b[23] != 0,
        };
        let count = u32_at(b, 24).ok_or_else(|| bad("truncated header"))? as usize;
        let posting_count = u32_at(b, 28).ok_or_else(|| bad("truncated header"))? as usize;
        let weight_scale = finite(f32_at(b, 32), "the weight scale")?;
        let threshold = finite(f32_at(b, 36), "the threshold")?;
        let margin = finite(f32_at(b, 40), "the margin")?;
        let short_threshold = finite(f32_at(b, 44), "the short-message threshold")?;
        let short_words = b[48];

        let mut at = HEADER;
        let mut classes = Vec::with_capacity(class_count);
        let mut slots = Vec::with_capacity(class_count);
        for _ in 0..class_count {
            let (name, next) = short_at(b, at)?;
            let count = *b.get(next).ok_or_else(|| bad("truncated class table"))?;
            at = next + 1;
            let mut fixed = BTreeMap::new();
            for _ in 0..count {
                let (key, next) = short_at(b, at)?;
                let (value, next) = short_at(b, next)?;
                at = next;
                fixed.insert(key, value);
            }
            classes.push(name);
            slots.push(fixed);
        }
        let biases = at;
        let vocab_offsets = biases + 4 * class_count;
        let vocab_bytes_start = vocab_offsets + 4 * (count + 1);
        let vocab_len = u32_at(b, vocab_offsets + 4 * count)
            .ok_or_else(|| bad("truncated vocabulary"))? as usize;
        let idf = vocab_bytes_start + vocab_len;
        let post_offsets = idf + 4 * count;
        let postings = post_offsets + 4 * (count + 1);
        let end = postings + 2 * posting_count;
        if end + 8 != b.len() {
            return Err(bad(format!(
                "sections end at byte {}, the file has {}",
                end + 8,
                b.len()
            )));
        }
        if u32_at(b, post_offsets + 4 * count).map(|n| n as usize) != Some(posting_count) {
            return Err(bad("weight table does not match its count"));
        }
        Ok(Self {
            kind,
            corpus_hash,
            features,
            weight_scale,
            threshold,
            margin,
            short_words,
            short_threshold,
            classes,
            slots,
            count,
            biases,
            vocab_offsets,
            vocab_bytes: vocab_bytes_start,
            idf,
            post_offsets,
            postings,
            end,
            bytes,
        })
    }

    /// Checks what [`open`](Self::open) does not: the checksum, that the
    /// vocabulary is sorted and distinct, that offsets grow and that every
    /// weight names a class.
    ///
    /// # Errors
    ///
    /// The first problem found.
    pub fn verify(&self) -> Result<(), ModelError> {
        let b: &[u8] = &self.bytes;
        let mut hash = Fnv::new();
        hash.bytes(&b[..self.end]);
        if u64_at(b, self.end) != Some(hash.finish()) {
            return Err(bad("checksum does not match"));
        }
        let mut previous: Option<&[u8]> = None;
        for i in 0..self.count {
            let feature = self.feature(i).ok_or_else(|| bad("malformed vocabulary"))?;
            if std::str::from_utf8(feature).is_err() {
                return Err(bad("a feature is not UTF-8"));
            }
            if previous.is_some_and(|p| p >= feature) {
                return Err(bad("vocabulary not sorted"));
            }
            previous = Some(feature);
            let (from, to) = self
                .posting_range(i)
                .ok_or_else(|| bad("malformed weights"))?;
            for p in from..to {
                let class = b.get(self.postings + 2 * p).copied();
                if class.is_none_or(|c| usize::from(c) >= self.classes.len()) {
                    return Err(bad("a weight names no class"));
                }
            }
        }
        Ok(())
    }

    /// What it classifies.
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// The hash of the corpus it was trained on.
    #[must_use]
    pub fn corpus_hash(&self) -> u64 {
        self.corpus_hash
    }

    /// The features it reads.
    #[must_use]
    pub fn features(&self) -> FeatureConfig {
        self.features
    }

    /// Class names, by index.
    #[must_use]
    pub fn classes(&self) -> &[String] {
        &self.classes
    }

    /// The slots an accepted guess of a class carries, as if a rule had
    /// fixed them; empty for most classes.
    #[must_use]
    pub fn class_slots(&self, class: usize) -> Option<&BTreeMap<String, String>> {
        self.slots.get(class)
    }

    /// The least probability for the top class to be accepted.
    #[must_use]
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// The least lead the top class needs over the runner-up.
    #[must_use]
    pub fn margin(&self) -> f32 {
        self.margin
    }

    /// The threshold for a message of `words` words: the short-message
    /// one when it has fewer than the model's `short_words`.
    #[must_use]
    pub fn threshold_for(&self, words: usize) -> f32 {
        if words < usize::from(self.short_words) {
            self.threshold.max(self.short_threshold)
        } else {
            self.threshold
        }
    }

    /// How many features the vocabulary has.
    #[must_use]
    pub fn vocabulary_len(&self) -> usize {
        self.count
    }

    /// The size of the file in bytes.
    #[must_use]
    pub fn file_len(&self) -> usize {
        self.bytes.len()
    }

    fn feature(&self, i: usize) -> Option<&[u8]> {
        let from = u32_at(&self.bytes, self.vocab_offsets + 4 * i)? as usize;
        let to = u32_at(&self.bytes, self.vocab_offsets + 4 * (i + 1))? as usize;
        self.bytes
            .get(self.vocab_bytes + from..self.vocab_bytes + to.max(from))
    }

    fn posting_range(&self, i: usize) -> Option<(usize, usize)> {
        let from = u32_at(&self.bytes, self.post_offsets + 4 * i)? as usize;
        let to = u32_at(&self.bytes, self.post_offsets + 4 * (i + 1))? as usize;
        let to = to.min((self.end - self.postings) / 2);
        Some((from.min(to), to))
    }

    /// The vocabulary index of a feature.
    fn lookup(&self, feature: &[u8]) -> Option<usize> {
        let (mut low, mut high) = (0, self.count);
        while low < high {
            let mid = low + (high - low) / 2;
            match self.feature(mid)?.cmp(feature) {
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
                Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    /// Every class with its probability for a match-form text, most
    /// probable first; ties keep class order.
    #[must_use]
    pub fn rank(&self, folded: &str) -> Vec<Scored> {
        let b: &[u8] = &self.bytes;
        // TF-IDF of the known features, L2-normalised; words the model
        // does not know count towards the unknown-word feature.
        let mut known: Vec<(usize, f64)> = Vec::new();
        let mut unknown = 0;
        let idf = |i: usize| f64::from(f32_at(b, self.idf + 4 * i).unwrap_or(0.0));
        for (feature, count) in extract(folded, self.features) {
            if let Some(i) = self.lookup(feature.as_bytes()) {
                known.push((i, f64::from(count) * idf(i)));
            } else if is_word(&feature) {
                unknown += count;
            }
        }
        if unknown > 0
            && let Some(i) = self.lookup(UNKNOWN_WORD.as_bytes())
        {
            known.push((i, f64::from(unknown) * idf(i)));
        }
        let norm = known.iter().map(|(_, x)| x * x).sum::<f64>().sqrt();
        let mut scores: Vec<f64> = (0..self.classes.len())
            .map(|c| f64::from(f32_at(b, self.biases + 4 * c).unwrap_or(0.0)))
            .collect();
        if norm > 0.0 {
            let scale = f64::from(self.weight_scale) / norm;
            for (i, x) in known {
                let Some((from, to)) = self.posting_range(i) else {
                    continue;
                };
                for p in from..to {
                    let at = self.postings + 2 * p;
                    let (Some(&class), Some(&weight)) = (b.get(at), b.get(at + 1)) else {
                        continue;
                    };
                    if let Some(score) = scores.get_mut(usize::from(class)) {
                        *score += x * scale * f64::from(i8::from_le_bytes([weight]));
                    }
                }
            }
        }
        softmax_ranked(&scores)
    }
}

/// Softmax of raw scores, ranked: most probable first, ties by index.
#[must_use]
pub fn softmax_ranked(scores: &[f64]) -> Vec<Scored> {
    let top = scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exps: Vec<f64> = scores.iter().map(|s| num::exp(s - top)).collect();
    let total: f64 = exps.iter().sum();
    let mut ranked: Vec<Scored> = exps
        .into_iter()
        .enumerate()
        .map(|(class, e)| Scored {
            class,
            probability: if total > 0.0 { e / total } else { 0.0 },
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.probability
            .total_cmp(&a.probability)
            .then(a.class.cmp(&b.class))
    });
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ModelData {
        ModelData {
            kind: Kind::Intent,
            corpus_hash: 42,
            features: FeatureConfig {
                min_n: 2,
                max_n: 3,
                words: true,
                word_pairs: false,
            },
            classes: vec![
                "a.x".to_owned(),
                "b.y".to_owned(),
                "out-of-scope".to_owned(),
            ],
            slots: vec![
                BTreeMap::new(),
                [("kind".to_owned(), "die".to_owned())].into(),
            ],
            biases: vec![0.0, 0.0, 0.5],
            weight_scale: 0.1,
            threshold: 0.5,
            margin: 0.2,
            short_words: 2,
            short_threshold: 0.9,
            vocabulary: vec![
                Entry {
                    feature: "\u{1}hora".to_owned(),
                    idf: 1.0,
                    weights: vec![(0, 100)],
                },
                Entry {
                    feature: "\u{1}dado".to_owned(),
                    idf: 1.0,
                    weights: vec![(1, 100), (0, -20)],
                },
            ],
        }
    }

    #[test]
    fn a_model_round_trips_and_ranks() {
        let bytes = data().encode().unwrap();
        let model = Model::open(bytes.clone()).unwrap();
        model.verify().unwrap();
        assert_eq!(model.kind(), Kind::Intent);
        assert_eq!(model.corpus_hash(), 42);
        assert_eq!(model.classes().len(), 3);
        assert_eq!(model.class_slots(1).unwrap()["kind"], "die");
        assert!(model.class_slots(2).unwrap().is_empty());
        assert_eq!(model.vocabulary_len(), 2);
        assert_eq!(model.file_len(), bytes.len());
        assert!((model.threshold_for(1) - 0.9).abs() < 1e-6);
        assert!((model.threshold_for(2) - 0.5).abs() < 1e-6);
        let ranked = model.rank("que hora es");
        assert_eq!(ranked[0].class, 0);
        let ranked = model.rank("tira un dado");
        assert_eq!(ranked[0].class, 1);
        let total: f64 = ranked.iter().map(|s| s.probability).sum();
        assert!((total - 1.0).abs() < 1e-12);
        // Nothing known: the biases decide.
        assert_eq!(model.rank("xyz")[0].class, 2);
        // Encoding is deterministic and independent of vocabulary order.
        let mut shuffled = data();
        shuffled.vocabulary.reverse();
        assert_eq!(shuffled.encode().unwrap(), bytes);
    }

    #[test]
    fn damaged_files_are_refused_or_caught() {
        let bytes = data().encode().unwrap();
        assert!(Model::open(b"nope".to_vec()).is_err());
        assert!(Model::open(bytes[..bytes.len() - 1].to_vec()).is_err());
        let mut version = bytes.clone();
        version[8] = 9;
        assert!(Model::open(version).unwrap_err().0.contains("version 9"));
        // A flipped bit inside a section opens, ranks without panicking,
        // and fails verification.
        let mut flipped = bytes.clone();
        let last = flipped.len() - 12;
        flipped[last] ^= 0x40;
        let model = Model::open(flipped).unwrap();
        let _ = model.rank("que hora es");
        assert!(model.verify().is_err());
        // Garbage of the right shape never panics either.
        for seed in 0..64u8 {
            let mut noisy = bytes.clone();
            for (i, byte) in noisy.iter_mut().enumerate().skip(HEADER) {
                *byte ^= seed
                    .wrapping_mul(31)
                    .wrapping_add(u8::try_from(i % 251).unwrap());
            }
            if let Ok(model) = Model::open(noisy) {
                let _ = model.rank("tira un dado y dime la hora");
                let _ = model.verify();
            }
        }
    }
}
