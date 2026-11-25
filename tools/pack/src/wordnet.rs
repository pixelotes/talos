//! The vocabulary pack from the WordNet family (docs/plans/v0.3.md, Slice C;
//! docs/research/wordnet-sources.md §2.3.1 and §4): Open English WordNet
//! 2025+ for concepts, English words, glosses, relations and the English
//! exception forms; the Spanish wordnet of the Multilingual Central
//! Repository, as packaged by the Open Multilingual Wordnet, for Spanish
//! words and glosses, joined through the Collaborative Interlingual Index;
//! Wikidata's Spanish descriptions as a third source of Spanish glosses.
//!
//! ```text
//! talos-pack wordnet DIR [OUT] [--unverified] [--wikidata FILE | --no-wikidata]
//! ```
//!
//! `DIR` holds the four downloaded source files (at its top level or one
//! directory down); each must have the SHA-256 sum below unless
//! `--unverified` is given, and the pack then says so in its manifest. The
//! fifth input, the Wikidata snapshot, is in the repository
//! (`knowledge/sources/`, [`crate::wikidata`]); `--wikidata FILE` names
//! another. Without the snapshot or the WordNet 3.1 map the builder stops,
//! unless `--no-wikidata` says to build without Wikidata. The XML is
//! streamed; everything is ordered by the sources' own order, so the same
//! files give the same bytes.
//!
//! What goes where:
//!
//! - **Concept**: one per English synset. Its id is the CILI id (`i46360`)
//!   when the synset has one, else the synset id (`oewn-00002935-r`). Kind
//!   `entity` for a synset with an instance hypernym (a named thing), else
//!   `class`.
//! - **English lexeme**: one per (lemma, part of speech) with `a` and `s`
//!   both `adj`; entries that share it are merged in document order. Its
//!   senses are ranked by their order in the lexical entry, which is
//!   WordNet's sense order (the commonest sense first); a lexeme that shares
//!   its match form with one of more senses starts at 1 ([`order_lexemes`]).
//! - **Spanish lexeme**: one per (lemma, part of speech) of the Spanish
//!   file, whitespace collapsed. The file gives no sense order, so a
//!   Spanish sense borrows the rank of its concept in the English word that
//!   most often translates the Spanish one (a guess; [`spanish_rank`]). A
//!   concept's Spanish words are written by that rank, the masculine before
//!   its feminine, then the most polysemous first.
//! - **Gloss**: the synset's definition on every English sense, under the
//!   source `oewn`; the Spanish definition on every Spanish sense of the
//!   concept, under `mcr-spa`. A Spanish sense with no such gloss gets
//!   Wikidata's Spanish description of the concept, under `wikidata`: the
//!   description of the items whose P8814 names the concept's WordNet 3.1
//!   synset (`ili-map-pwn31.tab` gives its CILI id); the lowest Q number
//!   when several differ; none when it is only one of the concept's own
//!   Spanish words, which defines nothing ([`wikidata_gloss`]). A sense's
//!   source is its gloss's source.
//! - **Forms**: the lemma, and the `Form` elements of the entry (the
//!   WordNet exception lists: "geese", "went", "better"), with features read
//!   from the part of speech and the ending ([`form_features`]).
//! - **Claims**: hypernym, instance hypernym, part, member and substance
//!   meronymy, causes, similar and antonym, each once per pair; antonyms are
//!   relations of senses in the source and become relations of their
//!   concepts here.
//!
//! Examples, pronunciations, the other relations and the Spanish examples
//! have no place in schema version 1 and are counted as dropped.
//!
//! The word-pair lexicon (KNOWLEDGE.md §7) is not built here: its source is
//! undecided. When it is, its pairs go in a pack of their own (`word_pair`
//! rows, `talos-pack build` already writes them from a `[lexicon]`), and the
//! senses it induces for Spanish words are added to this pack under a third
//! source with ranks after the wordnet's own.

use crate::wikidata;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use talos_core::knowledge::key;
use talos_core::knowledge::schema::{self, DbKind};

/// Open English WordNet 2025+, WN-LMF XML, gzipped.
pub(crate) const LMF: &str = "english-wordnet-2025-plus.xml.gz";
/// The CILI map from WordNet 3.0 offsets to ILI ids.
pub(crate) const ILI_MAP: &str = "ili-map-pwn30.tab";
/// The Spanish wordnet (MCR) in the OMW tab format.
pub(crate) const SPANISH: &str = "wn-data-spa.tab";
/// The CILI map from WordNet 3.1 offsets to ILI ids, which joins Wikidata's
/// P8814 to the concepts; read only when Wikidata is.
pub(crate) const ILI_MAP_31: &str = "ili-map-pwn31.tab";

/// The source files and their SHA-256 sums (wordnet-sources.md §2.3.1).
pub(crate) const EXPECTED: [(&str, &str); 4] = [
    (
        LMF,
        "31f4af16c54b532fd5484d4cc33aee588a31bb5b70683ae8197842fde5b586bc",
    ),
    (
        ILI_MAP,
        "13b6741395a8bdf20e5a19686ffa49167653696d4f523db17a79dce3211a6627",
    ),
    (
        SPANISH,
        "0ec37ce94ee2acc63ad6b120e1051f9841d172778cc24adc0023fb8dccf0a7cd",
    ),
    (
        ILI_MAP_31,
        "f6068d5506323b244a12d082206633630ad0176987caab3d044cbadb09223eb7",
    ),
];

/// Where the Wikidata snapshot is read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum WikidataSource {
    /// The snapshot at this path (and the WordNet 3.1 map in `DIR`).
    Snapshot(PathBuf),
    /// No Wikidata: the pack has two sources, as before.
    Off,
}

/// `source.added_at` and `claim.created_at`: the day the sources were
/// fetched and measured (wordnet-sources.md §2.3), fixed so that the build
/// is reproducible.
const DATE: i64 = 1_795_564_800; // 2026-11-25T00:00:00Z

/// The pack's manifest.
const ID: &str = "vocabulary";
const VERSION: &str = "1.0";
const TITLE: &str = "Core vocabulary: Open English WordNet 2025+ and the MCR Spanish wordnet";
const TITLE_WIKIDATA: &str = "Core vocabulary: Open English WordNet 2025+, the MCR Spanish wordnet \
                              and Wikidata descriptions";
/// Every source's licence applies to its part; no single outbound licence
/// is claimed (wordnet-sources.md §3). CC0 adds no obligation; it is named
/// for the part it covers.
const LICENSE: &str = "CC-BY-4.0 AND CC-BY-3.0 AND WordNet";
const LICENSE_WIKIDATA: &str = "CC-BY-4.0 AND CC-BY-3.0 AND WordNet AND CC0-1.0";

/// The notice of wordnet-sources.md §4, for this edition, with what was
/// changed; with Wikidata's part when the pack uses it.
pub(crate) fn notice(wikidata: bool) -> String {
    let wikidata_source = if wikidata {
        "\
- Wikidata (https://www.wikidata.org/): the Spanish descriptions of items that name a
  WordNet 3.1 synset (property P8814), written by Wikidata contributors and dedicated to
  the public domain under CC0 1.0 (https://creativecommons.org/publicdomain/zero/1.0/).
  They are short labels, not dictionary definitions, and are attached to concepts
  through the WordNet 3.1 synset id.
"
    } else {
        ""
    };
    let wikidata_terms = if wikidata {
        ", the parts derived from Wikidata under CC0, which adds no obligation"
    } else {
        ""
    };
    let sources = if wikidata {
        "`oewn`, `mcr-spa` or `wikidata`"
    } else {
        "`oewn` or `mcr-spa`"
    };
    let join = if wikidata {
        "whose WordNet 3.0 map joins the Spanish wordnet to Open English WordNet and whose\nWordNet 3.1 map joins the Wikidata descriptions to it"
    } else {
        "whose WordNet 3.0 map\njoins the Spanish wordnet to Open English WordNet"
    };
    let wikidata_change = if wikidata {
        "; Spanish senses with no MCR\ndefinition given the Wikidata description of the same synset (the lowest item\nnumber when several differ)"
    } else {
        ""
    };
    format!(
        "\
This knowledge pack contains data derived from:

- Open English WordNet (2025+ edition), Copyright 2025 the Open English WordNet
  contributors, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/).
  Open English WordNet is derived from Princeton WordNet.
  https://en-word.net/
- Princeton WordNet 3.0, Copyright 2006 by Princeton University. All rights reserved.
  WordNet is provided \"AS IS\" with no warranties; Princeton University's name may not
  be used in advertising or publicity pertaining to distribution of this data.
  (WordNet License, https://wordnet.princeton.edu/license-and-commercial-use)
- Multilingual Central Repository 3.0 (Spanish wordnet), Gonzalez-Agirre, Laparra and
  Rigau, GWC 2012, licensed under CC BY 3.0 (https://creativecommons.org/licenses/by/3.0/).
  https://adimen.ehu.eus/web/MCR
{wikidata_source}
The data was converted to a SQLite schema, merged by concept, and reduced to the
fields Talos uses; no endorsement by the original authors is implied.

Each part keeps its source's licence: the parts derived from Open English WordNet
are under CC BY 4.0 with the WordNet notice retained, the parts derived from the
Multilingual Central Repository under CC BY 3.0{wikidata_terms}. Every sense and gloss names its
source ({sources}). Concept identifiers are those of the Collaborative
Interlingual Index (https://github.com/globalwordnet/cili), {join}.

Changes: Spanish synsets joined to Open English WordNet synsets through CILI;
examples, pronunciations, syntactic frames and the derivational, domain and
pertainym relations left out; antonyms moved from word senses to concepts;
Spanish lemmas with collapsed whitespace; sense ranks for Spanish words
computed from the English sense order{wikidata_change}."
    )
}

/// Where each file of `EXPECTED` is: `dir/NAME` or `dir/*/NAME`.
fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let mut subdirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    subdirs.sort();
    subdirs
        .into_iter()
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// The SHA-256 of a file, in lower-case hex.
fn sha256(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 1 << 16];
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    }))
}

/// The files, found and checked.
struct Files {
    lmf: PathBuf,
    ili: PathBuf,
    spanish: PathBuf,
    /// The WordNet 3.1 map, when Wikidata is read.
    ili31: Option<PathBuf>,
    /// `name sha256` per file, for the manifest.
    sums: Vec<String>,
    /// Whether every sum was the expected one.
    verified: bool,
}

fn locate(dir: &Path, unverified: bool, wikidata: bool) -> Result<Files, String> {
    let mut paths = Vec::new();
    let mut sums = Vec::new();
    let mut verified = true;
    for (name, expected) in EXPECTED {
        if name == ILI_MAP_31 && !wikidata {
            continue;
        }
        let path = find(dir, name).ok_or_else(|| {
            format!(
                "{name} is not in {} (nor one directory down); see docs/DEVELOPMENT.md, \
                 \"The vocabulary pack\"{}",
                dir.display(),
                if name == ILI_MAP_31 {
                    "; pass --no-wikidata to build without Wikidata descriptions"
                } else {
                    ""
                }
            )
        })?;
        let sum = sha256(&path)?;
        if sum != expected {
            if !unverified {
                return Err(format!(
                    "{}: SHA-256 {sum}, expected {expected}; refusing an unknown file \
                     (pass --unverified to build from it anyway)",
                    path.display()
                ));
            }
            verified = false;
            eprintln!(
                "talos-pack: warning: {} is not the file the pack was written for",
                path.display()
            );
        }
        sums.push(format!("{name} {sum}"));
        paths.push(path);
    }
    let mut paths = paths.into_iter();
    let mut next = || paths.next().ok_or("missing path");
    Ok(Files {
        lmf: next()?,
        ili: next()?,
        spanish: next()?,
        ili31: if wikidata { Some(next()?) } else { None },
        sums,
        verified,
    })
}

/// What the build counted, kept and dropped, in the order it found them.
#[derive(Debug, Default)]
pub(crate) struct Report {
    lines: Vec<String>,
    counts: BTreeMap<&'static str, i64>,
}

impl Report {
    fn line(&mut self, text: impl Into<String>) {
        self.lines.push(text.into());
    }

    fn count(&mut self, what: &'static str, n: usize) {
        let n = i64::try_from(n).unwrap_or(i64::MAX);
        self.counts.insert(what, n);
        self.lines.push(format!("{what}: {n}"));
    }

    /// A count by name, for tests.
    #[cfg(test)]
    #[must_use]
    fn get(&self, what: &str) -> Option<i64> {
        self.counts.get(what).copied()
    }

    /// The report as text, one line per item.
    #[must_use]
    pub(crate) fn text(&self) -> String {
        let mut out = self.lines.join("\n");
        out.push('\n');
        out
    }
}

/// A `LexicalEntry`.
#[derive(Debug, Default)]
struct Entry {
    id: String,
    lemma: String,
    pos: String,
    forms: Vec<String>,
    /// (sense id, synset id), in the entry's order.
    senses: Vec<(String, String)>,
}

/// A `Synset`.
#[derive(Debug, Default)]
struct Synset {
    id: String,
    ili: String,
    pos: String,
    members: Vec<String>,
    definitions: Vec<String>,
    /// (relType, target synset), in order.
    relations: Vec<(String, String)>,
}

/// What the LMF file holds that the pack uses, and counts of the rest.
#[derive(Debug, Default)]
struct Lmf {
    lexicon: Vec<(String, String)>,
    entries: Vec<Entry>,
    synsets: Vec<Synset>,
    /// (source sense, target sense) of every sense-level antonym.
    antonyms: Vec<(String, String)>,
    /// Sense relations of other types, by type.
    other_sense_relations: BTreeMap<String, usize>,
    /// Elements that carry nothing the schema holds, by name.
    unused: BTreeMap<String, usize>,
}

fn attributes(e: &BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for a in e.attributes() {
        let a = a.map_err(|e| e.to_string())?;
        let k = a.key.as_ref().to_owned();
        let v = a
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?
            .into_owned();
        out.push((k, v));
    }
    Ok(out)
}

fn get(attrs: &[(String, String)], name: &str) -> String {
    attrs
        .iter()
        .find(|(k, _)| k == name)
        .map_or_else(String::new, |(_, v)| v.clone())
}

/// Reads a WN-LMF document, streaming: entries, synsets with their
/// definitions and relations, sense-level antonyms. Text of other elements
/// is skipped.
#[allow(clippy::too_many_lines, reason = "one match over the element names")]
fn read_lmf(input: impl BufRead) -> Result<Lmf, String> {
    let mut reader = Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut lmf = Lmf::default();
    let mut entry: Option<Entry> = None;
    let mut synset: Option<Synset> = None;
    let mut sense: Option<String> = None;
    let mut text: Option<String> = None;
    loop {
        let at = reader.buffer_position();
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("XML at byte {at}: {e}"))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name = e.name().as_ref().to_owned();
                let attrs = attributes(&e)?;
                match name.as_str() {
                    "LexicalResource" => {}
                    "Lexicon" => lmf.lexicon = attrs,
                    "LexicalEntry" => {
                        entry = Some(Entry {
                            id: get(&attrs, "id"),
                            ..Entry::default()
                        });
                    }
                    "Lemma" => {
                        if let Some(entry) = entry.as_mut() {
                            entry.lemma = get(&attrs, "writtenForm");
                            entry.pos = get(&attrs, "partOfSpeech");
                        }
                    }
                    "Form" => {
                        if let Some(entry) = entry.as_mut() {
                            entry.forms.push(get(&attrs, "writtenForm"));
                        }
                    }
                    "Sense" => {
                        let id = get(&attrs, "id");
                        if let Some(entry) = entry.as_mut() {
                            entry.senses.push((id.clone(), get(&attrs, "synset")));
                        }
                        if !empty {
                            sense = Some(id);
                        }
                    }
                    "SenseRelation" => {
                        let kind = get(&attrs, "relType");
                        if kind == "antonym"
                            && let Some(from) = &sense
                        {
                            lmf.antonyms.push((from.clone(), get(&attrs, "target")));
                        } else {
                            *lmf.other_sense_relations.entry(kind).or_default() += 1;
                        }
                    }
                    "Synset" => {
                        let members = get(&attrs, "members");
                        let s = Synset {
                            id: get(&attrs, "id"),
                            ili: get(&attrs, "ili"),
                            pos: get(&attrs, "partOfSpeech"),
                            members: members.split_whitespace().map(str::to_owned).collect(),
                            ..Synset::default()
                        };
                        if empty {
                            lmf.synsets.push(s);
                        } else {
                            synset = Some(s);
                        }
                    }
                    "Definition" if synset.is_some() && !empty => text = Some(String::new()),
                    "SynsetRelation" => {
                        if let Some(s) = synset.as_mut() {
                            s.relations
                                .push((get(&attrs, "relType"), get(&attrs, "target")));
                        }
                    }
                    other => *lmf.unused.entry(other.to_owned()).or_default() += 1,
                }
            }
            Event::End(e) => match e.name().as_ref() {
                "LexicalEntry" => {
                    if let Some(done) = entry.take() {
                        lmf.entries.push(done);
                    }
                }
                "Sense" => sense = None,
                "Synset" => {
                    if let Some(done) = synset.take() {
                        lmf.synsets.push(done);
                    }
                }
                "Definition" => {
                    if let (Some(t), Some(s)) = (text.take(), synset.as_mut()) {
                        s.definitions.push(t.trim().to_owned());
                    }
                }
                _ => {}
            },
            Event::Text(t) => {
                if let Some(out) = text.as_mut() {
                    out.push_str(&t.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                if let Some(out) = text.as_mut() {
                    if let Some(c) = r.resolve_char_ref().map_err(|e| e.to_string())? {
                        out.push(c);
                    } else if let Some(s) = quick_xml::escape::resolve_predefined_entity(&r) {
                        out.push_str(s);
                    } else {
                        return Err(format!("XML at byte {at}: unknown entity &{};", &*r));
                    }
                }
            }
            Event::CData(c) => {
                if let Some(out) = text.as_mut() {
                    out.push_str(&c);
                }
            }
            Event::Eof => break,
            Event::Decl(_) | Event::DocType(_) | Event::Comment(_) | Event::PI(_) => {}
        }
        buf.clear();
    }
    if entry.is_some() || synset.is_some() {
        return Err("XML ends inside an element".to_owned());
    }
    Ok(lmf)
}

/// The CILI map: WordNet 3.0 `offset-pos` → ILI id.
fn read_ili(input: impl BufRead) -> Result<HashMap<String, String>, String> {
    let mut map = HashMap::new();
    for line in input.lines() {
        let line = line.map_err(|e| e.to_string())?;
        let mut fields = line.split('\t');
        if let (Some(ili), Some(offset)) = (fields.next(), fields.next())
            && !ili.starts_with('#')
        {
            map.insert(offset.trim().to_owned(), ili.trim().to_owned());
        }
    }
    Ok(map)
}

/// One synset of the Spanish file.
#[derive(Debug, Default)]
struct SpanishSynset {
    lemmas: Vec<String>,
    /// (index, text).
    definitions: Vec<(String, String)>,
    examples: usize,
}

/// The Spanish file: synsets by `offset-pos` in file order, the header, and
/// counts of what it holds.
#[derive(Debug, Default)]
struct Spanish {
    header: String,
    synsets: BTreeMap<String, SpanishSynset>,
    lemma_lines: usize,
    collapsed: usize,
    empty: usize,
    unknown_lines: usize,
    /// How often each sequence of one to four words occurs in the file's
    /// definitions and examples, in the match form: the only frequency
    /// the Spanish source offers ([`text_frequency`]).
    frequency: HashMap<String, usize>,
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The words of a text in the match form.
fn tokens(text: &str) -> Vec<String> {
    key(text)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Counts every sequence of one to four words of `text`.
fn count_ngrams(text: &str, counts: &mut HashMap<String, usize>) {
    let words = tokens(text);
    for n in 1..=4 {
        for window in words.windows(n) {
            *counts.entry(window.join(" ")).or_default() += 1;
        }
    }
}

/// How often a lemma occurs in the Spanish definitions and examples: a
/// corpus frequency of sorts, used only to order a concept's Spanish words
/// ("perro" occurs in many definitions, "canino" in few).
fn text_frequency(lemma: &str, counts: &HashMap<String, usize>) -> usize {
    counts.get(&tokens(lemma).join(" ")).copied().unwrap_or(0)
}

fn read_spanish(input: impl BufRead) -> Result<Spanish, String> {
    let mut out = Spanish::default();
    for line in input.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if let Some(header) = line.strip_prefix('#') {
            if out.header.is_empty() {
                header.trim().clone_into(&mut out.header);
            }
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            [offset, "spa:lemma", lemma] => {
                out.lemma_lines += 1;
                let clean = collapse(lemma);
                if clean.is_empty() || key(&clean).is_empty() {
                    out.empty += 1;
                    continue;
                }
                if clean != *lemma {
                    out.collapsed += 1;
                }
                let s = out.synsets.entry((*offset).to_owned()).or_default();
                if !s.lemmas.contains(&clean) {
                    s.lemmas.push(clean);
                }
            }
            [offset, "spa:def", index, text] => {
                let text = collapse(text);
                count_ngrams(&text, &mut out.frequency);
                if !text.is_empty() {
                    out.synsets
                        .entry((*offset).to_owned())
                        .or_default()
                        .definitions
                        .push(((*index).to_owned(), text));
                }
            }
            [offset, "spa:exe", .., text] => {
                count_ngrams(text, &mut out.frequency);
                out.synsets
                    .entry((*offset).to_owned())
                    .or_default()
                    .examples += 1;
            }
            [""] => {}
            _ => out.unknown_lines += 1,
        }
    }
    Ok(out)
}

fn open_text(path: &Path) -> Result<BufReader<File>, String> {
    File::open(path)
        .map(|f| BufReader::with_capacity(1 << 16, f))
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The schema's part of speech for a WordNet one.
fn part_of_speech(pos: &str) -> Option<&'static str> {
    match pos {
        "n" => Some("n"),
        "v" => Some("v"),
        "a" | "s" => Some("adj"),
        "r" => Some("adv"),
        _ => None,
    }
}

/// The features of an exception form, from the part of speech and the
/// ending: the WordNet exception lists do not say which form a form is.
/// Nouns: plural. Verbs: `ger` for `-ing`, `pres,3sg` for a form in `-s`
/// ("is", "has", "does"), `pres` for "am" and "are", else `past` (past or
/// past participle: "went", "gone", "abetted"). Adjectives and adverbs:
/// `sup` for `-st` ("best", "worst", "least"), else `cmp`.
fn form_features(pos: &str, form: &str) -> &'static str {
    match pos {
        "v" => {
            if form.ends_with("ing") {
                "ger"
            } else if form == "am" || form == "are" {
                "pres"
            } else if form.ends_with('s') {
                "pres,3sg"
            } else {
                "past"
            }
        }
        "adj" | "adv" => {
            if form.ends_with("st") {
                "sup"
            } else {
                "cmp"
            }
        }
        _ => "pl",
    }
}

fn is_ili(ili: &str) -> bool {
    ili.len() > 1 && ili.starts_with('i') && ili[1..].bytes().all(|b| b.is_ascii_digit())
}

/// A lexeme as it will be written.
#[derive(Debug)]
struct Lexeme {
    lang: &'static str,
    lemma: String,
    pos: &'static str,
    /// (form in the match form, features).
    forms: Vec<(String, &'static str)>,
    /// The synsets (indexes) of its senses, best first, no repeats.
    senses: Vec<usize>,
}

/// Row ids and rank offsets of the lexemes. Lexemes that share a match form
/// in one language ("dog" noun and verb, "CAT" and "cat", "árbol" and the
/// unaccented "arbol" the Spanish file also has) are one typed word with
/// no order between them in the sources. The main one keeps its ranks and
/// the others start at 1, so that a common word's first sense is not tied
/// with an abbreviation, a proper name or a rarer part of speech. The main
/// one is the one with the most senses, a noun's counted twice ("tree" the
/// noun, two senses, before the verb, four; "run" the verb, 41, before the
/// noun, 16), then the one in lower case. Ids follow (language, match form,
/// that order), so a tie at equal rank lists the main lexeme first.
fn order_lexemes(lexemes: &[Lexeme]) -> (Vec<i64>, Vec<i64>) {
    let mut groups: BTreeMap<(&str, String), Vec<usize>> = BTreeMap::new();
    for (i, l) in lexemes.iter().enumerate() {
        groups.entry((l.lang, key(&l.lemma))).or_default().push(i);
    }
    let (mut ids, mut offsets) = (vec![0; lexemes.len()], vec![0; lexemes.len()]);
    let mut next = 0;
    for members in groups.values_mut() {
        members.sort_by_key(|&i| {
            let l = &lexemes[i];
            let noun = l.pos == "n";
            (
                std::cmp::Reverse(l.senses.len() * if noun { 2 } else { 1 }),
                !noun,
                l.lemma.chars().any(char::is_uppercase),
                i,
            )
        });
        for (k, &i) in members.iter().enumerate() {
            next += 1;
            ids[i] = next;
            offsets[i] = i64::from(k > 0);
        }
    }
    (ids, offsets)
}

/// The English word that most often translates a Spanish one: among the
/// English lexemes of its concepts, one written in the same case (a
/// common word is not translated by a proper name: "pan" → *bread*, not
/// *Pan*), found on the most of its concepts ("coche" → *car*, on two of
/// four), on which those concepts rank best ("río" → *river*, rank 0, not
/// *stream*, rank 1), with the most senses.
fn pivot(word: &Lexeme, english_of: &[Vec<usize>], lexemes: &[Lexeme]) -> Option<usize> {
    let upper = |l: &Lexeme| l.lemma.chars().next().is_some_and(char::is_uppercase);
    // English lexeme → (concepts shared, sum of their ranks in it).
    let mut found: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for &s in &word.senses {
        for &e in &english_of[s] {
            let rank = lexemes[e].senses.iter().position(|x| *x == s).unwrap_or(0);
            let entry = found.entry(e).or_default();
            entry.0 += 1;
            entry.1 += rank;
        }
    }
    found
        .into_iter()
        .max_by_key(|&(e, (count, ranks))| {
            (
                upper(&lexemes[e]) == upper(word),
                count,
                std::cmp::Reverse(ranks),
                lexemes[e].senses.len(),
                std::cmp::Reverse(e),
            )
        })
        .map(|(e, _)| e)
}

/// The rank of a Spanish sense, before the lexeme's offset. The Spanish
/// file has no sense order, so it is borrowed from the English one: the
/// concept's rank in the word's [`pivot`] when the pivot names it ("coche"
/// → *car* 0, *railcar* 1), else after every sense of the pivot, by the
/// concept's best rank in any of its English words. A guess from the only
/// signal the sources give.
fn spanish_rank(
    pivot: Option<usize>,
    synset: usize,
    english_of: &[Vec<usize>],
    lexemes: &[Lexeme],
) -> usize {
    let position = |e: usize| lexemes[e].senses.iter().position(|x| *x == synset);
    let Some(p) = pivot else {
        return 0;
    };
    position(p).unwrap_or_else(|| {
        lexemes[p].senses.len()
            + english_of[synset]
                .iter()
                .filter_map(|&e| position(e))
                .min()
                .unwrap_or(0)
    })
}

/// Whether a Spanish word is the feminine of another word of the same
/// concept ("perra" beside "perro"): the masculine is the dictionary form
/// and names the concept first.
fn feminine_beside(lemma: &str, others: &[&str]) -> bool {
    lemma
        .strip_suffix('a')
        .is_some_and(|stem| others.iter().any(|o| o.strip_suffix('o') == Some(stem)))
}

/// How a concept's Spanish words of equal rank are ordered: a plural of
/// another of them last, a feminine after its masculine, then the word most
/// frequent in the Spanish definitions, with the most concepts, the shorter.
type SpanishOrder<'a> = (
    bool,
    bool,
    std::cmp::Reverse<usize>,
    std::cmp::Reverse<usize>,
    usize,
    &'a str,
);

/// Whether a Spanish word is the plural of another word of the same
/// concept: the Spanish file has some plurals as lemmas ("ríos" beside
/// "río", "perros" beside "perro"); they name the concept last.
fn plural_beside(lemma: &str, others: &[&str]) -> bool {
    let k = key(lemma);
    others.iter().any(|o| {
        let o = key(o);
        o != k && (k.strip_suffix('s') == Some(&o) || k.strip_suffix("es") == Some(&o))
    })
}

/// The ids of the sources in the pack's `source` table.
const SOURCE_OEWN: i64 = 1;
const SOURCE_MCR: i64 = 2;
const SOURCE_WIKIDATA: i64 = 3;

/// The CILI id of a synset of a WordNet map; an adjective is also tried as
/// the satellite `-s` the map may write for it.
fn lookup_ili<'a>(map: &'a HashMap<String, String>, synset: &str) -> Option<&'a String> {
    map.get(synset).or_else(|| {
        let stem = synset.strip_suffix("-a")?;
        map.get(&format!("{stem}-s"))
    })
}

/// Wikidata's descriptions by concept: each row's WordNet 3.1 synset id to a
/// CILI id through `ili31`, then to the OEWN synset (index) that carries it.
/// Every description of a synset, the lowest Q number first, equal ones once;
/// empty ones are dropped. The report counts what was read, lost and
/// resolved.
fn describe(
    rows: &[wikidata::Row],
    ili31: &HashMap<String, String>,
    by_ili: &HashMap<&str, usize>,
    r: &mut Report,
) -> BTreeMap<usize, Vec<String>> {
    let (mut empty, mut no_ili, mut elsewhere) = (0, Vec::new(), Vec::new());
    // CILI id → (Q number, description).
    let mut by_concept: BTreeMap<&str, Vec<(u64, &str)>> = BTreeMap::new();
    for row in rows {
        if row.description.is_empty() {
            empty += 1;
        } else if let Some(id) = lookup_ili(ili31, &row.synset) {
            by_concept
                .entry(id)
                .or_default()
                .push((row.item, &row.description));
        } else {
            no_ili.push(format!("Q{} {}", row.item, row.synset));
        }
    }
    let mut out = BTreeMap::new();
    let mut conflicts = 0;
    for (id, mut candidates) in by_concept {
        candidates.sort_unstable();
        let mut texts: Vec<String> = Vec::new();
        for (_, text) in candidates {
            if !texts.iter().any(|t| t == text) {
                texts.push(text.to_owned());
            }
        }
        conflicts += usize::from(texts.len() > 1);
        match by_ili.get(id) {
            Some(&synset) => {
                out.insert(synset, texts);
            }
            None => elsewhere.push(id.to_owned()),
        }
    }
    r.count("wikidata: rows read", rows.len());
    r.line(format!(
        "DROPPED wikidata rows with an empty description: {empty}"
    ));
    r.line(format!(
        "DROPPED wikidata rows with no CILI id in the WordNet 3.1 map: {}{}",
        no_ili.len(),
        examples(&no_ili, 5)
    ));
    r.count(
        "wikidata: concepts described (by CILI id)",
        out.len() + elsewhere.len(),
    );
    r.line(format!(
        "DROPPED wikidata concepts whose CILI id no OEWN synset carries: {}{}",
        elsewhere.len(),
        examples(&elsewhere, 5)
    ));
    r.count(
        "wikidata: concepts with several different descriptions (lowest Q number chosen)",
        conflicts,
    );
    r.count(
        "wikidata: concepts described that are in the pack",
        out.len(),
    );
    out
}

/// The description a concept's Spanish senses get: the first of its
/// descriptions (lowest Q number first) that is not, in the match form, one
/// of the concept's own Spanish words. A description that only repeats a word
/// ("lengua" for *lengua*) defines nothing; no other judgement of what makes
/// a definition is made.
fn wikidata_gloss<'a>(descriptions: &'a [String], words: &[String]) -> Option<&'a str> {
    let own: Vec<String> = words.iter().map(|w| key(w)).collect();
    descriptions
        .iter()
        .find(|d| !own.contains(&key(d)))
        .map(String::as_str)
}

/// The predicates of the pack: (name, inverse, transitive, symmetric).
const PREDICATES: &[(&str, Option<&str>, bool, bool)] = &[
    ("is_a", Some("has_kind"), true, false),
    ("instance_of", Some("has_instance"), false, false),
    ("part_of", Some("has_part"), true, false),
    ("member_of", Some("has_member"), false, false),
    ("substance_of", Some("has_substance"), false, false),
    ("causes", Some("caused_by"), true, false),
    ("similar_to", None, false, true),
    ("antonym_of", None, false, true),
];

/// A synset relation as a claim: (predicate, whether the synset carrying the
/// relation is the claim's subject). The inverse relations the file also
/// writes (`hyponym`, `holo_*`/`mero_*`, `is_caused_by`) give the same claims and
/// are deduplicated.
fn claim_of(rel: &str) -> Option<(&'static str, bool)> {
    Some(match rel {
        "hypernym" => ("is_a", true),
        "hyponym" => ("is_a", false),
        "instance_hypernym" => ("instance_of", true),
        "instance_hyponym" => ("instance_of", false),
        "holo_part" => ("part_of", true),
        "mero_part" => ("part_of", false),
        "holo_member" => ("member_of", true),
        "mero_member" => ("member_of", false),
        "holo_substance" => ("substance_of", true),
        "mero_substance" => ("substance_of", false),
        "causes" => ("causes", true),
        "is_caused_by" => ("causes", false),
        "similar" => ("similar_to", true),
        _ => return None,
    })
}

fn sql(e: &rusqlite::Error) -> String {
    e.to_string()
}

/// Up to `n` examples, for the report.
fn examples<'a>(items: impl IntoIterator<Item = &'a String>, n: usize) -> String {
    let v: Vec<&str> = items.into_iter().take(n).map(String::as_str).collect();
    if v.is_empty() {
        String::new()
    } else {
        format!(" (e.g. {})", v.join(", "))
    }
}

/// The Wikidata side of a build: its rows and the WordNet 3.1 map.
struct Wikidata {
    rows: Vec<wikidata::Row>,
    /// WordNet 3.1 `offset-pos` → ILI id.
    ili31: HashMap<String, String>,
}

/// Builds the vocabulary pack from the sources in `dir` into `out`, and
/// writes its notice beside it (`OUT` with the extension `NOTICE`). Refuses
/// files whose checksum is not the expected one unless `unverified`. Reads
/// the Wikidata snapshot, and the WordNet 3.1 map from `dir`, unless
/// `wikidata` is [`WikidataSource::Off`]; a snapshot or map that is missing
/// is an error then, never a silent omission.
///
/// # Errors
///
/// A message for a person when a file is missing, unknown or unreadable, or
/// a statement fails.
pub(crate) fn build(
    dir: &Path,
    out: &Path,
    unverified: bool,
    wikidata: &WikidataSource,
) -> Result<Report, String> {
    let mut files = locate(dir, unverified, *wikidata != WikidataSource::Off)?;
    let wikidata = match (wikidata, &files.ili31) {
        (WikidataSource::Snapshot(path), Some(ili31)) => {
            if !path.is_file() {
                return Err(format!(
                    "the Wikidata snapshot {} is not a file; see knowledge/sources/README.md \
                     (pass --no-wikidata to build without Wikidata descriptions)",
                    path.display()
                ));
            }
            let name = path.file_name().map_or_else(
                || "wikidata".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            files.sums.push(format!("{name} {}", sha256(path)?));
            let rows =
                wikidata::read(open_text(path)?).map_err(|e| format!("{}: {e}", path.display()))?;
            Some(Wikidata {
                rows,
                ili31: read_ili(open_text(ili31)?)?,
            })
        }
        _ => None,
    };
    let lmf = read_lmf(BufReader::with_capacity(
        1 << 16,
        GzDecoder::new(open_text(&files.lmf)?),
    ))?;
    let ili = read_ili(open_text(&files.ili)?)?;
    let spanish = read_spanish(open_text(&files.spanish)?)?;
    let (conn, report) = assemble(&lmf, &ili, &spanish, wikidata.as_ref(), &files)?;
    write_pack(&conn, out)?;
    let notice_path = out.with_extension("NOTICE");
    std::fs::write(&notice_path, format!("{}\n", notice(wikidata.is_some())))
        .map_err(|e| format!("{}: {e}", notice_path.display()))?;
    Ok(report)
}

/// Writes an in-memory pack to `out` with `VACUUM INTO`, after `ANALYZE`,
/// as `build_file` does for a hand-written pack.
fn write_pack(conn: &Connection, out: &Path) -> Result<(), String> {
    conn.execute_batch("ANALYZE").map_err(|e| sql(&e))?;
    if let Some(parent) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    match std::fs::remove_file(out) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("{}: {e}", out.display())),
    }
    let path = out.to_str().ok_or("not a UTF-8 path")?;
    conn.execute("VACUUM INTO ?1", [path])
        .map_err(|e| format!("{}: {e}", out.display()))?;
    Ok(())
}

/// Joins the sources and writes every row into an in-memory pack.
#[allow(
    clippy::too_many_lines,
    reason = "the build, step by step, with its report"
)]
fn assemble(
    lmf: &Lmf,
    ili: &HashMap<String, String>,
    spanish: &Spanish,
    wikidata: Option<&Wikidata>,
    files: &Files,
) -> Result<(Connection, Report), String> {
    let mut r = Report::default();
    r.line(format!(
        "sources{}: {}",
        if files.verified {
            ""
        } else {
            " (NOT the expected files)"
        },
        files.sums.join("; ")
    ));
    let lexicon = |k: &str| get(&lmf.lexicon, k);
    r.line(format!(
        "Open English WordNet: lexicon {} version {}, licence {}",
        lexicon("id"),
        lexicon("version"),
        lexicon("license")
    ));
    r.line(format!("Spanish file header: {}", spanish.header));

    // Synsets → concepts.
    r.count("en synsets in the source", lmf.synsets.len());
    let mut synset_index: HashMap<&str, usize> = HashMap::new();
    let mut by_ili: HashMap<&str, usize> = HashMap::new();
    let mut concept_ids: Vec<String> = Vec::with_capacity(lmf.synsets.len());
    let mut duplicate_ili = Vec::new();
    let mut no_ili = 0;
    for (i, s) in lmf.synsets.iter().enumerate() {
        synset_index.insert(&s.id, i);
        if is_ili(&s.ili) {
            if by_ili.insert(&s.ili, i).is_some() {
                duplicate_ili.push(s.ili.clone());
            }
            concept_ids.push(s.ili.clone());
        } else {
            no_ili += 1;
            concept_ids.push(s.id.clone());
        }
    }
    if !duplicate_ili.is_empty() {
        return Err(format!(
            "{} ILI ids on more than one synset{}",
            duplicate_ili.len(),
            examples(&duplicate_ili, 5)
        ));
    }
    r.count(
        "en synsets without a CILI id (concept id = synset id, no Spanish side)",
        no_ili,
    );

    // Entries → English lexemes, merged by (lemma, part of speech).
    r.count("en lexical entries in the source", lmf.entries.len());
    let mut lexemes: Vec<Lexeme> = Vec::new();
    let mut lexeme_of: HashMap<(String, &'static str), usize> = HashMap::new();
    let mut entry_lexeme: HashMap<&str, usize> = HashMap::new();
    let mut sense_synset: HashMap<&str, usize> = HashMap::new();
    let (mut merged, mut bad_pos, mut empty_key) = (Vec::new(), Vec::new(), Vec::new());
    let (mut unknown_synset, mut repeated_sense) = (Vec::new(), 0);
    let (mut forms_kept, mut forms_same, mut forms_repeated) = (0, 0, 0);
    for e in &lmf.entries {
        let Some(pos) = part_of_speech(&e.pos) else {
            bad_pos.push(format!("{} ({})", e.id, e.pos));
            continue;
        };
        let form = key(&e.lemma);
        if e.lemma.trim().is_empty() || form.is_empty() {
            empty_key.push(e.id.clone());
            continue;
        }
        let k = (e.lemma.clone(), pos);
        let index = if let Some(&i) = lexeme_of.get(&k) {
            merged.push(e.id.clone());
            i
        } else {
            lexeme_of.insert(k, lexemes.len());
            lexemes.push(Lexeme {
                lang: "en",
                lemma: e.lemma.clone(),
                pos,
                forms: Vec::new(),
                senses: Vec::new(),
            });
            lexemes.len() - 1
        };
        entry_lexeme.insert(&e.id, index);
        let lexeme = &mut lexemes[index];
        for f in &e.forms {
            let fk = key(f);
            if fk.is_empty() || fk == form {
                forms_same += 1;
            } else if lexeme.forms.iter().any(|(k, _)| *k == fk) {
                forms_repeated += 1;
            } else {
                let features = form_features(pos, &fk);
                lexeme.forms.push((fk, features));
                forms_kept += 1;
            }
        }
        for (sense, synset) in &e.senses {
            let Some(&s) = synset_index.get(synset.as_str()) else {
                unknown_synset.push(sense.clone());
                continue;
            };
            sense_synset.insert(sense, s);
            if lexeme.senses.contains(&s) {
                repeated_sense += 1;
            } else {
                lexeme.senses.push(s);
            }
        }
    }
    let english_lexemes = lexemes.len();
    r.count("en lexemes", english_lexemes);
    r.line(format!(
        "en entries merged into an earlier lexeme of the same lemma and part of speech: {}{}",
        merged.len(),
        examples(&merged, 5)
    ));
    r.line(format!(
        "DROPPED en entries with an unknown part of speech: {}{}",
        bad_pos.len(),
        examples(&bad_pos, 5)
    ));
    r.line(format!(
        "DROPPED en entries whose lemma has an empty match form: {}{}",
        empty_key.len(),
        examples(&empty_key, 5)
    ));
    r.line(format!(
        "DROPPED en senses naming a synset not in the file: {}{}",
        unknown_synset.len(),
        examples(&unknown_synset, 5)
    ));
    r.line(format!(
        "DROPPED en senses repeating a (lexeme, synset) pair after merging: {repeated_sense}"
    ));
    r.count(
        "en exception forms (Form elements) in the source",
        lmf.entries.iter().map(|e| e.forms.len()).sum(),
    );
    r.count("en exception forms kept", forms_kept);
    r.line(format!(
        "DROPPED en exception forms equal to the lemma in the match form: {forms_same}; \
         repeated on a merged lexeme: {forms_repeated}"
    ));

    // English senses per synset, in member order.
    let mut english_of: Vec<Vec<usize>> = vec![Vec::new(); lmf.synsets.len()];
    for (li, l) in lexemes.iter().enumerate() {
        for &s in &l.senses {
            english_of[s].push(li);
        }
    }
    let mut not_members = 0;
    for (s, synset) in lmf.synsets.iter().enumerate() {
        let order: Vec<usize> = synset
            .members
            .iter()
            .filter_map(|m| entry_lexeme.get(m.as_str()).copied())
            .collect();
        let list = &mut english_of[s];
        let position = |l: &usize| order.iter().position(|o| o == l).unwrap_or(usize::MAX);
        not_members += list.iter().filter(|l| position(l) == usize::MAX).count();
        list.sort_by_key(|l| position(l));
    }
    r.line(format!(
        "en senses whose lexeme is not in its synset's members (written last): {not_members}"
    ));
    let wordless = english_of.iter().filter(|l| l.is_empty()).count();
    r.line(format!("en synsets with no English word: {wordless}"));

    // Definitions.
    let mut glossless = 0;
    let mut several = 0;
    let gloss_english: Vec<Option<String>> = lmf
        .synsets
        .iter()
        .map(|s| {
            let defs: Vec<&str> = s
                .definitions
                .iter()
                .map(String::as_str)
                .filter(|d| !d.is_empty())
                .collect();
            if defs.is_empty() {
                glossless += 1;
            }
            if defs.len() > 1 {
                several += 1;
            }
            (!defs.is_empty()).then(|| defs.join("; "))
        })
        .collect();
    r.line(format!(
        "en synsets without a definition: {glossless}; with several (joined with \"; \"): {several}"
    ));

    // The Spanish join.
    let with_lemma = spanish
        .synsets
        .values()
        .filter(|s| !s.lemmas.is_empty())
        .count();
    r.count("es synset ids in the source", spanish.synsets.len());
    r.count("es synsets with a lemma", with_lemma);
    r.count("es lemma lines", spanish.lemma_lines);
    r.line(format!(
        "es lemmas whose whitespace was collapsed: {}; DROPPED empty lemmas: {}; \
         DROPPED unreadable lines: {}",
        spanish.collapsed, spanish.empty, spanish.unknown_lines
    ));
    let mut spanish_of: Vec<Vec<String>> = vec![Vec::new(); lmf.synsets.len()];
    let mut gloss_spanish: Vec<Option<String>> = vec![None; lmf.synsets.len()];
    let (mut joined, mut via_s, mut no_ili_es, mut not_in_oewn) = (0, 0, Vec::new(), Vec::new());
    let mut lost_by_pos: BTreeMap<String, usize> = BTreeMap::new();
    let (mut glosses_in, mut glosses_kept, mut gloss_conflicts, mut gloss_no_word) = (0, 0, 0, 0);
    let mut examples_es = 0;
    let mut into_same = 0;
    for (offset, s) in &spanish.synsets {
        examples_es += s.examples;
        let has_gloss = !s.definitions.is_empty();
        if has_gloss {
            glosses_in += 1;
        }
        if s.lemmas.is_empty() {
            if has_gloss {
                gloss_no_word += 1;
            }
            continue;
        }
        let pos = offset.rsplit('-').next().unwrap_or("");
        let mut found = ili.get(offset.as_str());
        if found.is_none() && pos == "a" {
            let satellite = format!("{}-s", &offset[..offset.len() - 2]);
            found = ili.get(&satellite);
            if found.is_some() {
                via_s += 1;
            }
        }
        let Some(id) = found else {
            no_ili_es.push(offset.clone());
            *lost_by_pos.entry(pos.to_owned()).or_default() += 1;
            continue;
        };
        let Some(&synset) = by_ili.get(id.as_str()) else {
            not_in_oewn.push(format!("{offset}→{id}"));
            *lost_by_pos.entry(pos.to_owned()).or_default() += 1;
            continue;
        };
        joined += 1;
        if !spanish_of[synset].is_empty() {
            into_same += 1;
        }
        for lemma in &s.lemmas {
            if !spanish_of[synset].contains(lemma) {
                spanish_of[synset].push(lemma.clone());
            }
        }
        if has_gloss {
            let mut defs = s.definitions.clone();
            defs.sort();
            let text = defs
                .into_iter()
                .map(|(_, t)| t)
                .collect::<Vec<_>>()
                .join("; ");
            if gloss_spanish[synset].is_some() {
                gloss_conflicts += 1;
            } else {
                gloss_spanish[synset] = Some(text);
                glosses_kept += 1;
            }
        }
    }
    r.count("es synsets joined to an OEWN synset", joined);
    r.line(format!(
        "es synsets joined only through the satellite suffix -s: {via_s}"
    ));
    r.line(format!(
        "DROPPED es synsets with no CILI id in the WordNet 3.0 map: {}{}",
        no_ili_es.len(),
        examples(&no_ili_es, 5)
    ));
    r.line(format!(
        "DROPPED es synsets whose CILI id no OEWN synset carries: {}{}",
        not_in_oewn.len(),
        examples(&not_in_oewn, 5)
    ));
    r.line(format!(
        "es synsets lost, by part of speech: {lost_by_pos:?}"
    ));
    r.line(format!(
        "es synsets joined onto an OEWN synset another es synset already reached (lemmas merged): {into_same}"
    ));
    r.count("es synsets with a definition in the source", glosses_in);
    r.count("es definitions kept", glosses_kept);
    r.line(format!(
        "DROPPED es definitions of synsets with no Spanish lemma: {gloss_no_word}; \
         of a second es synset on the same concept: {gloss_conflicts}; of synsets lost in the join: {}",
        glosses_in - glosses_kept - gloss_no_word - gloss_conflicts
    ));
    r.line(format!(
        "DROPPED es examples (no place in the schema): {examples_es}"
    ));

    // Wikidata's descriptions for the Spanish senses with no MCR gloss.
    let mut gloss_source: Vec<i64> = vec![SOURCE_MCR; lmf.synsets.len()];
    if let Some(w) = wikidata {
        let described = describe(&w.rows, &w.ili31, &by_ili, &mut r);
        let (mut kept, mut no_words, mut as_lemma, mut given) = (0, 0, 0, 0);
        for (synset, candidates) in described {
            if gloss_spanish[synset].is_some() {
                kept += 1;
            } else if spanish_of[synset].is_empty() {
                no_words += 1;
            } else if let Some(text) = wikidata_gloss(&candidates, &spanish_of[synset]) {
                gloss_spanish[synset] = Some(text.to_owned());
                gloss_source[synset] = SOURCE_WIKIDATA;
                given += 1;
            } else {
                as_lemma += 1;
            }
        }
        r.count(
            "wikidata: described concepts that already have an MCR gloss (kept)",
            kept,
        );
        r.count(
            "wikidata: described concepts with no Spanish word (unused)",
            no_words,
        );
        r.count(
            "wikidata: concepts whose descriptions all equal one of their own Spanish words (skipped)",
            as_lemma,
        );
        r.count(
            "wikidata: concepts newly glossed (a Spanish word, no MCR gloss)",
            given,
        );
    }

    // Spanish lexemes, by (lemma, part of speech), sorted.
    let mut es_keys: BTreeMap<(String, &'static str), Vec<usize>> = BTreeMap::new();
    let mut es_empty_pos = 0;
    for (s, lemmas) in spanish_of.iter().enumerate() {
        let Some(pos) = part_of_speech(&lmf.synsets[s].pos) else {
            es_empty_pos += lemmas.len();
            continue;
        };
        for lemma in lemmas {
            es_keys.entry((lemma.clone(), pos)).or_default().push(s);
        }
    }
    if es_empty_pos > 0 {
        r.line(format!(
            "DROPPED es lemmas on a synset with an unknown part of speech: {es_empty_pos}"
        ));
    }
    let mut es_lexeme: HashMap<(&str, &str), usize> = HashMap::new();
    for ((lemma, pos), synsets) in &es_keys {
        es_lexeme.insert((lemma.as_str(), pos), lexemes.len());
        lexemes.push(Lexeme {
            lang: "es",
            lemma: lemma.clone(),
            pos,
            forms: Vec::new(),
            senses: synsets.clone(),
        });
    }
    r.count("es lexemes", lexemes.len() - english_lexemes);
    let odd: Vec<String> = lexemes[english_lexemes..]
        .iter()
        .filter(|l| {
            l.lemma.chars().any(|c| {
                c.is_alphabetic() && !c.is_ascii_alphabetic() && !"áéíóúüñÁÉÍÓÚÜÑ".contains(c)
            })
        })
        .map(|l| l.lemma.clone())
        .collect();
    r.line(format!(
        "es lemmas with letters outside Spanish spelling (kept as written): {}{}",
        odd.len(),
        examples(&odd, 10)
    ));

    // Ranks and row ids.
    let (ids, offsets) = order_lexemes(&lexemes);
    let pivots: Vec<Option<usize>> = lexemes
        .iter()
        .enumerate()
        .map(|(i, l)| {
            (i >= english_lexemes)
                .then(|| pivot(l, &english_of, &lexemes))
                .flatten()
        })
        .collect();
    r.line(format!(
        "lexemes sharing a match form with a lexeme of more senses (ranks start at 1): {}",
        offsets.iter().filter(|o| **o > 0).count()
    ));

    // Write.
    let mut conn = Connection::open_in_memory().map_err(|e| sql(&e))?;
    let tx = conn.transaction().map_err(|e| sql(&e))?;
    schema::migrate(&tx, "main", DbKind::Pack).map_err(|e| e.to_string())?;
    let sums = files.sums.join("\n");
    let verified = if files.verified { "yes" } else { "no" };
    let notice = notice(wikidata.is_some());
    for (k, v) in [
        ("id", ID),
        ("version", VERSION),
        (
            "title",
            if wikidata.is_some() {
                TITLE_WIKIDATA
            } else {
                TITLE
            },
        ),
        ("languages", "es en"),
        (
            "license",
            if wikidata.is_some() {
                LICENSE_WIKIDATA
            } else {
                LICENSE
            },
        ),
        ("notice", notice.as_str()),
        ("source_files", sums.as_str()),
        ("source_files_verified", verified),
    ] {
        tx.execute("INSERT INTO meta (key, value) VALUES (?1, ?2)", [k, v])
            .map_err(|e| sql(&e))?;
    }
    tx.execute(
        "INSERT INTO source (id, kind, name, license, url, added_at) VALUES \
         (1, 'import', 'oewn', 'CC-BY-4.0 AND WordNet', 'https://en-word.net/', ?1), \
         (2, 'import', 'mcr-spa', 'CC-BY-3.0', 'https://adimen.ehu.eus/web/MCR', ?1)",
        [DATE],
    )
    .map_err(|e| sql(&e))?;
    if wikidata.is_some() {
        tx.execute(
            "INSERT INTO source (id, kind, name, license, url, added_at) VALUES \
             (3, 'import', 'wikidata', 'CC0-1.0', 'https://www.wikidata.org/', ?1)",
            [DATE],
        )
        .map_err(|e| sql(&e))?;
    }
    let mut predicate_id: HashMap<&str, i64> = HashMap::new();
    for (i, (name, inverse, transitive, symmetric)) in PREDICATES.iter().enumerate() {
        let id = i64::try_from(i + 1).map_err(|e| e.to_string())?;
        predicate_id.insert(name, id);
        tx.execute(
            "INSERT INTO predicate (id, name, inverse, transitive, symmetric) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, name, inverse, transitive, symmetric],
        )
        .map_err(|e| sql(&e))?;
    }
    let mut entities = 0;
    {
        let mut concept = tx
            .prepare("INSERT INTO concept (id, kind) VALUES (?1, ?2)")
            .map_err(|e| sql(&e))?;
        for (s, synset) in lmf.synsets.iter().enumerate() {
            let entity = synset
                .relations
                .iter()
                .any(|(rel, _)| rel == "instance_hypernym");
            entities += usize::from(entity);
            concept
                .execute(params![
                    concept_ids[s],
                    if entity { "entity" } else { "class" }
                ])
                .map_err(|e| sql(&e))?;
        }
        let mut lexeme = tx
            .prepare("INSERT INTO lexeme (id, lang, lemma, pos) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|e| sql(&e))?;
        let mut form = tx
            .prepare("INSERT INTO form (lexeme_id, form, features) VALUES (?1, ?2, ?3)")
            .map_err(|e| sql(&e))?;
        let mut by_id: Vec<usize> = (0..lexemes.len()).collect();
        by_id.sort_by_key(|&i| ids[i]);
        for i in by_id {
            let (id, l) = (ids[i], &lexemes[i]);
            lexeme
                .execute(params![id, l.lang, l.lemma, l.pos])
                .map_err(|e| sql(&e))?;
            form.execute(params![id, key(&l.lemma), Option::<&str>::None])
                .map_err(|e| sql(&e))?;
            for (f, features) in &l.forms {
                form.execute(params![id, f, features])
                    .map_err(|e| sql(&e))?;
            }
        }
        let mut sense = tx
            .prepare(
                "INSERT INTO sense (lexeme_id, concept_id, gloss, rank, source_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )
            .map_err(|e| sql(&e))?;
        let (mut english_senses, mut spanish_senses, mut english_glossed) = (0, 0, 0);
        let (mut spanish_glossed, mut wikidata_glossed) = (0, 0);
        let mut plurals = 0;
        for s in 0..lmf.synsets.len() {
            for &l in &english_of[s] {
                let rank = lexemes[l].senses.iter().position(|x| *x == s).unwrap_or(0);
                let rank = i64::try_from(rank).map_err(|e| e.to_string())? + offsets[l];
                sense
                    .execute(params![
                        ids[l],
                        concept_ids[s],
                        gloss_english[s],
                        rank,
                        SOURCE_OEWN
                    ])
                    .map_err(|e| sql(&e))?;
                english_senses += 1;
                english_glossed += usize::from(gloss_english[s].is_some());
            }
            let Some(pos) = part_of_speech(&lmf.synsets[s].pos) else {
                continue;
            };
            let words: Vec<usize> = spanish_of[s]
                .iter()
                .filter_map(|lemma| es_lexeme.get(&(lemma.as_str(), pos)).copied())
                .collect();
            let lemmas: Vec<&str> = words.iter().map(|&l| lexemes[l].lemma.as_str()).collect();
            // (rank, then the order the concept's words are written in).
            let mut ranked: Vec<(i64, SpanishOrder<'_>, usize)> = Vec::with_capacity(words.len());
            for &l in &words {
                let w = &lexemes[l];
                let rank = spanish_rank(pivots[l], s, &english_of, &lexemes);
                let rank = i64::try_from(rank).map_err(|e| e.to_string())? + offsets[l];
                let plural = plural_beside(&w.lemma, &lemmas);
                plurals += usize::from(plural);
                let order = (
                    plural,
                    feminine_beside(&w.lemma, &lemmas),
                    std::cmp::Reverse(text_frequency(&w.lemma, &spanish.frequency)),
                    std::cmp::Reverse(w.senses.len()),
                    w.lemma.split(' ').count(),
                    w.lemma.as_str(),
                );
                ranked.push((rank, order, l));
            }
            ranked.sort_unstable();
            for (rank, _, l) in ranked {
                sense
                    .execute(params![
                        ids[l],
                        concept_ids[s],
                        gloss_spanish[s],
                        rank,
                        gloss_source[s]
                    ])
                    .map_err(|e| sql(&e))?;
                spanish_senses += 1;
                if gloss_spanish[s].is_some() {
                    if gloss_source[s] == SOURCE_WIKIDATA {
                        wikidata_glossed += 1;
                    } else {
                        spanish_glossed += 1;
                    }
                }
            }
        }
        r.count("concepts", lmf.synsets.len());
        r.count("concepts of kind entity (instances)", entities);
        r.count(
            "concepts with Spanish words",
            spanish_of.iter().filter(|w| !w.is_empty()).count(),
        );
        r.count("en senses", english_senses);
        r.count("es senses", spanish_senses);
        r.line(format!("es senses of a word that is the plural of another word of its concept (written last): {plurals}"));
        r.count("en senses with a gloss (source oewn)", english_glossed);
        r.count("es senses with a gloss (source mcr-spa)", spanish_glossed);
        if wikidata.is_some() {
            r.count(
                "wikidata: es senses given a gloss (source wikidata)",
                wikidata_glossed,
            );
        }
        r.count(
            "concepts with a Spanish gloss",
            gloss_spanish.iter().filter(|g| g.is_some()).count(),
        );
        r.count(
            "concepts with an English gloss",
            gloss_english.iter().filter(|g| g.is_some()).count(),
        );
        let forms: usize = lexemes.iter().map(|l| 1 + l.forms.len()).sum();
        r.count("forms (lemmas and exceptions)", forms);
        let mut by_features: BTreeMap<&str, usize> = BTreeMap::new();
        for l in &lexemes {
            for (_, f) in &l.forms {
                *by_features.entry(f).or_default() += 1;
            }
        }
        r.line(format!("exception forms by features: {by_features:?}"));
    }

    // Claims.
    let mut claims: Vec<(usize, &'static str, usize)> = Vec::new();
    let mut seen: BTreeSet<(usize, &'static str, usize)> = BTreeSet::new();
    let mut dropped_rel: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut dangling, mut self_loops) = (0, 0);
    let mut push = |c: (usize, &'static str, usize)| {
        if c.0 == c.2 {
            self_loops += 1;
        } else if seen.insert(c) {
            claims.push(c);
        }
    };
    for (s, synset) in lmf.synsets.iter().enumerate() {
        for (rel, target) in &synset.relations {
            let Some((predicate, forward)) = claim_of(rel) else {
                *dropped_rel.entry(rel).or_default() += 1;
                continue;
            };
            let Some(&t) = synset_index.get(target.as_str()) else {
                dangling += 1;
                continue;
            };
            push(if forward {
                (s, predicate, t)
            } else {
                (t, predicate, s)
            });
        }
    }
    let mut antonym_dangling = 0;
    for (from, to) in &lmf.antonyms {
        match (
            sense_synset.get(from.as_str()),
            sense_synset.get(to.as_str()),
        ) {
            (Some(&a), Some(&b)) => push((a, "antonym_of", b)),
            _ => antonym_dangling += 1,
        }
    }
    let antonym_claims = claims.iter().filter(|c| c.1 == "antonym_of").count();
    {
        let mut claim = tx
            .prepare(
                "INSERT INTO claim (id, subject, predicate, object, confidence, source_id, created_at) \
                 VALUES (?1, ?2, ?3, ?4, 1.0, 1, ?5)",
            )
            .map_err(|e| sql(&e))?;
        for (i, (s, p, o)) in claims.iter().enumerate() {
            let id = i64::try_from(i + 1).map_err(|e| e.to_string())?;
            claim
                .execute(params![
                    id,
                    concept_ids[*s],
                    predicate_id[p],
                    concept_ids[*o],
                    DATE
                ])
                .map_err(|e| sql(&e))?;
        }
    }
    tx.commit().map_err(|e| sql(&e))?;
    r.count("claims", claims.len());
    let mut by_predicate: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, p, _) in &claims {
        *by_predicate.entry(p).or_default() += 1;
    }
    for (p, n) in &by_predicate {
        r.line(format!("claims {p}: {n}"));
    }
    r.line(format!(
        "antonyms: {} sense-level links in the source, {antonym_claims} concept-level claims \
         after collapsing; DROPPED with an unknown sense: {antonym_dangling}",
        lmf.antonyms.len()
    ));
    r.line(format!(
        "DROPPED synset relations to an unknown synset: {dangling}; self-relations after the \
         sense→concept collapse: {self_loops}"
    ));
    r.line(format!(
        "DROPPED synset relations of types the pack does not hold: {dropped_rel:?}"
    ));
    r.line(format!(
        "DROPPED sense relations other than antonym: {:?}",
        lmf.other_sense_relations
    ));
    r.line(format!(
        "DROPPED elements with no place in the schema: {:?}",
        lmf.unused
    ));
    Ok((conn, r))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LMF_SAMPLE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE LexicalResource SYSTEM "http://globalwordnet.github.io/schemas/WN-LMF-1.3.dtd">
<LexicalResource xmlns:dc="https://globalwordnet.github.io/schemas/dc/">
  <Lexicon id="oewn" label="t" language="en" email="e" license="https://creativecommons.org/licenses/by/4.0" version="2025+" url="u">
    <LexicalEntry id="oewn-dog-n">
      <Lemma writtenForm="dog" partOfSpeech="n"><Pronunciation>dɒɡ</Pronunciation></Lemma>
      <Sense id="oewn-dog__1" synset="oewn-02-n"/>
      <Sense id="oewn-dog__2" synset="oewn-03-n"/>
    </LexicalEntry>
    <LexicalEntry id="oewn-goose-n">
      <Lemma writtenForm="goose" partOfSpeech="n"/>
      <Form writtenForm="geese"/>
      <Sense id="oewn-goose__1" synset="oewn-04-n"/>
    </LexicalEntry>
    <LexicalEntry id="oewn-good-a">
      <Lemma writtenForm="good" partOfSpeech="a"/>
      <Form writtenForm="better"/><Form writtenForm="best"/>
      <Sense id="oewn-good__1" synset="oewn-05-a">
        <SenseRelation relType="antonym" target="oewn-bad__1"/>
        <SenseRelation relType="derivation" target="oewn-dog__1"/>
      </Sense>
    </LexicalEntry>
    <LexicalEntry id="oewn-good-s">
      <Lemma writtenForm="good" partOfSpeech="s"/>
      <Sense id="oewn-good__2" synset="oewn-07-s"/>
    </LexicalEntry>
    <LexicalEntry id="oewn-bad-a">
      <Lemma writtenForm="bad" partOfSpeech="a"/>
      <Sense id="oewn-bad__1" synset="oewn-06-a">
        <SenseRelation relType="antonym" target="oewn-good__1"/>
      </Sense>
    </LexicalEntry>
    <LexicalEntry id="oewn-dog-v">
      <Lemma writtenForm="dog" partOfSpeech="v"/>
      <Sense id="oewn-dog__3" synset="oewn-08-v"/>
    </LexicalEntry>
    <LexicalEntry id="oewn-cad-n">
      <Lemma writtenForm="cad" partOfSpeech="n"/>
      <Sense id="oewn-cad__1" synset="oewn-03-n"/>
    </LexicalEntry>
    <Synset id="oewn-01-n" ili="i1" members="" partOfSpeech="n" lexfile="noun.Tops">
      <Definition>an animal</Definition>
    </Synset>
    <Synset id="oewn-02-n" ili="i2" members="oewn-dog-n" partOfSpeech="n" lexfile="noun.animal">
      <Definition>a member of the genus Canis &amp; friends&apos; kin</Definition>
      <SynsetRelation relType="hypernym" target="oewn-01-n"/>
      <Example>the dog barked</Example>
    </Synset>
    <Synset id="oewn-03-n" ili="i3" members="oewn-cad-n oewn-dog-n" partOfSpeech="n" lexfile="noun.person">
      <Definition>someone who is morally reprehensible</Definition>
    </Synset>
    <Synset id="oewn-04-n" ili="in" members="oewn-goose-n" partOfSpeech="n" lexfile="noun.animal">
      <Definition>a web-footed bird</Definition>
      <ILIDefinition>a web-footed bird</ILIDefinition>
      <SynsetRelation relType="hypernym" target="oewn-01-n"/>
    </Synset>
    <Synset id="oewn-05-a" ili="i5" members="oewn-good-a" partOfSpeech="a" lexfile="adj.all">
      <Definition>having desirable qualities</Definition>
      <SynsetRelation relType="similar" target="oewn-07-s"/>
    </Synset>
    <Synset id="oewn-06-a" ili="i6" members="oewn-bad-a" partOfSpeech="a" lexfile="adj.all">
      <Definition>having undesirable qualities</Definition>
    </Synset>
    <Synset id="oewn-07-s" ili="i7" members="oewn-good-s" partOfSpeech="s" lexfile="adj.all">
      <Definition>ample</Definition>
      <SynsetRelation relType="similar" target="oewn-05-a"/>
    </Synset>
    <Synset id="oewn-08-v" ili="i8" members="oewn-dog-v" partOfSpeech="v" lexfile="verb.motion">
      <Definition>go after with the intent to catch</Definition>
    </Synset>
  </Lexicon>
</LexicalResource>
"#;

    const ILI_SAMPLE: &str =
        "i1\t00000001-n\ni2\t00000002-n\ni3\t00000003-n\ni5\t00000005-a\ni7\t00000007-s\n";

    const SPANISH_SAMPLE: &str = "# Multilingual Central Repository\tspa\thttp://x\tCC BY 3.0\n\
        00000002-n\tspa:lemma\tperro\n\
        00000002-n\tspa:lemma\tcan\n\
        00000002-n\tspa:def\t0\tanimal  doméstico\n\
        00000002-n\tspa:exe\t0\tel perro ladra\n\
        00000003-n\tspa:lemma\tperro\n\
        00000007-a\tspa:lemma\tbueno\n\
        00000009-n\tspa:lemma\tnada\n\
        00000010-n\tspa:def\t0\tsin palabra\n";

    /// The WordNet 3.1 offsets of the sample (arbitrary, as the others).
    const ILI31_SAMPLE: &str = "i1\t10000001-n\ni2\t10000002-n\ni3\t10000003-n\n\
        i5\t10000005-a\ni7\t10000007-s\ni99\t10000099-n\n";

    /// Q10 and Q20 describe the same synset (Q10 wins); Q5 describes a concept
    /// the MCR glosses (kept); Q6 describes "bueno" with the word itself
    /// (skipped); Q7 has no Spanish word to give a gloss to; Q8's synset is
    /// not in the map, Q9's CILI id not in the pack, Q11's description empty.
    const WIKIDATA_SAMPLE: &str = "?i\t?s\t?d\n\
        <http://www.wikidata.org/entity/Q20>\t\"10000003-n\"\t\"persona despreciable\"@es\n\
        <http://www.wikidata.org/entity/Q10>\t\"10000003-n\"\t\"sinvergüenza\"@es\n\
        <http://www.wikidata.org/entity/Q5>\t\"10000002-n\"\t\"perro doméstico\"@es\n\
        <http://www.wikidata.org/entity/Q6>\t\"10000007-a\"\t\"Bueno\"@es\n\
        <http://www.wikidata.org/entity/Q7>\t\"10000001-n\"\t\"animal\"@es\n\
        <http://www.wikidata.org/entity/Q8>\t\"99999999-n\"\t\"nada\"@es\n\
        <http://www.wikidata.org/entity/Q9>\t\"10000099-n\"\t\"fuera\"@es\n\
        <http://www.wikidata.org/entity/Q11>\t\"10000005-a\"\t\"\"@es\n";

    fn sample_with(wikidata: bool) -> (Connection, Report) {
        let lmf = read_lmf(LMF_SAMPLE.as_bytes()).unwrap();
        let ili = read_ili(ILI_SAMPLE.as_bytes()).unwrap();
        let spanish = read_spanish(SPANISH_SAMPLE.as_bytes()).unwrap();
        let files = Files {
            lmf: PathBuf::new(),
            ili: PathBuf::new(),
            spanish: PathBuf::new(),
            ili31: None,
            sums: Vec::new(),
            verified: true,
        };
        let wikidata = wikidata.then(|| Wikidata {
            rows: wikidata::read(WIKIDATA_SAMPLE.as_bytes()).unwrap(),
            ili31: read_ili(ILI31_SAMPLE.as_bytes()).unwrap(),
        });
        assemble(&lmf, &ili, &spanish, wikidata.as_ref(), &files).unwrap()
    }

    fn sample() -> (Connection, Report) {
        sample_with(true)
    }

    fn rows(conn: &Connection, query: &str) -> Vec<String> {
        let mut st = conn.prepare(query).unwrap();
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn the_sample_is_read_joined_and_written() {
        let (conn, report) = sample();
        assert_eq!(
            rows(
                &conn,
                "SELECT id || ' ' || kind FROM concept ORDER BY rowid"
            ),
            [
                "i1 class",
                "i2 class",
                "i3 class",
                "oewn-04-n class",
                "i5 class",
                "i6 class",
                "i7 class",
                "i8 class"
            ]
        );
        // "dog" the verb shares its match form with the noun, which has
        // more senses: its ranks start at 1.
        assert_eq!(
            rows(
                &conn,
                "SELECT l.pos || ' ' || s.concept_id || ' ' || s.rank FROM sense s \
                 JOIN lexeme l ON l.id = s.lexeme_id WHERE l.lemma = 'dog' ORDER BY l.id, s.rank"
            ),
            ["n i2 0", "n i3 1", "v i8 1"]
        );
        // "good" a and s are one adjective; its senses in entry order.
        assert_eq!(
            rows(
                &conn,
                "SELECT s.concept_id || ' ' || s.rank FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 WHERE l.lemma = 'good' ORDER BY s.rank"
            ),
            ["i5 0", "i7 1"]
        );
        // Entities and glosses, with the entities of the XML resolved.
        let gloss = rows(
            &conn,
            "SELECT gloss FROM sense WHERE concept_id = 'i2' AND source_id = 1",
        );
        assert_eq!(gloss, ["a member of the genus Canis & friends' kin"]);
        // The satellite joins through -s; "perro" has two senses, dog first.
        assert_eq!(
            rows(
                &conn,
                "SELECT s.concept_id || ' ' || s.rank || ' ' || ifnull(s.gloss, '-') FROM sense s \
                 JOIN lexeme l ON l.id = s.lexeme_id WHERE l.lang = 'es' AND l.lemma = 'perro' \
                 ORDER BY s.rank, s.id"
            ),
            ["i2 0 animal doméstico", "i3 1 sinvergüenza"]
        );
        assert_eq!(
            rows(
                &conn,
                "SELECT concept_id FROM sense s JOIN lexeme l ON l.id = s.lexeme_id WHERE l.lemma = 'bueno'"
            ),
            ["i7"]
        );
        // The more polysemous Spanish word names the concept first.
        assert_eq!(
            rows(
                &conn,
                "SELECT l.lemma FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 WHERE s.concept_id = 'i2' AND l.lang = 'es' ORDER BY s.rank, s.id"
            ),
            ["perro", "can"]
        );
        // Exceptions with features; the lemma row has none.
        assert_eq!(
            rows(
                &conn,
                "SELECT form || ' ' || ifnull(features, '-') FROM form \
                 WHERE form IN ('goose', 'geese', 'good', 'better', 'best') ORDER BY rowid"
            ),
            ["good -", "better cmp", "best sup", "goose -", "geese pl"]
        );
        assert!(feminine_beside("perra", &["perro", "can"]));
        assert!(!feminine_beside("casa", &["cosa", "caso de"]));
        // Claims: hypernyms, similar both ways once each, antonyms collapsed.
        assert_eq!(
            rows(
                &conn,
                "SELECT c.subject || ' ' || p.name || ' ' || c.object FROM claim c \
                 JOIN predicate p ON p.id = c.predicate ORDER BY c.id"
            ),
            [
                "i2 is_a i1",
                "oewn-04-n is_a i1",
                "i5 similar_to i7",
                "i7 similar_to i5",
                "i5 antonym_of i6",
                "i6 antonym_of i5"
            ]
        );
        assert_eq!(report.get("es synsets joined to an OEWN synset"), Some(3));
        assert_eq!(report.get("es definitions kept"), Some(1));
        let text = report.text();
        assert!(
            text.contains("no CILI id in the WordNet 3.0 map: 1"),
            "{text}"
        );
        assert!(text.contains("\"derivation\": 1"), "{text}");
        assert!(text.contains("\"Example\": 1"), "{text}");
    }

    #[test]
    fn wikidata_fills_only_the_spanish_senses_without_a_gloss() {
        let (conn, report) = sample();
        // The lowest Q number wins; the MCR gloss of i2 stays; "bueno" has
        // only a description equal to itself: none.
        assert_eq!(
            rows(
                &conn,
                "SELECT s.concept_id || ' ' || l.lemma || ' ' || x.name || ' ' || ifnull(s.gloss, '-') \
                 FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 JOIN source x ON x.id = s.source_id WHERE l.lang = 'es' \
                 ORDER BY s.concept_id, l.lemma"
            ),
            [
                "i2 can mcr-spa animal doméstico",
                "i2 perro mcr-spa animal doméstico",
                "i3 perro wikidata sinvergüenza",
                "i7 bueno mcr-spa -"
            ]
        );
        // English senses are untouched.
        assert_eq!(
            count(
                &conn,
                "SELECT count(*) FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 WHERE l.lang = 'en' AND s.source_id <> 1"
            ),
            0
        );
        assert_eq!(
            rows(
                &conn,
                "SELECT name || ' ' || license FROM source ORDER BY id"
            ),
            [
                "oewn CC-BY-4.0 AND WordNet",
                "mcr-spa CC-BY-3.0",
                "wikidata CC0-1.0"
            ]
        );
        assert_eq!(report.get("wikidata: rows read"), Some(8));
        assert_eq!(
            report.get("wikidata: concepts described (by CILI id)"),
            Some(5)
        );
        assert_eq!(
            report.get("wikidata: concepts described that are in the pack"),
            Some(4)
        );
        assert_eq!(
            report.get(
                "wikidata: concepts with several different descriptions (lowest Q number chosen)"
            ),
            Some(1)
        );
        assert_eq!(
            report.get("wikidata: described concepts that already have an MCR gloss (kept)"),
            Some(1)
        );
        assert_eq!(
            report.get("wikidata: described concepts with no Spanish word (unused)"),
            Some(1)
        );
        assert_eq!(
            report.get("wikidata: es senses given a gloss (source wikidata)"),
            Some(1)
        );
        assert_eq!(
            report.get("wikidata: concepts newly glossed (a Spanish word, no MCR gloss)"),
            Some(1)
        );
        let text = report.text();
        assert!(
            text.contains("no CILI id in the WordNet 3.1 map: 1 (e.g. Q8 99999999-n)"),
            "{text}"
        );
        assert!(text.contains("empty description: 1"), "{text}");
        assert!(
            text.contains("no OEWN synset carries: 1 (e.g. i99)"),
            "{text}"
        );
        assert_eq!(
            wikidata_gloss(&["Perro".to_owned()], &["perro".to_owned()]),
            None
        );
    }

    #[test]
    fn without_wikidata_the_pack_has_two_sources_and_the_old_licence() {
        let (conn, report) = sample_with(false);
        assert_eq!(
            rows(&conn, "SELECT name FROM source ORDER BY id"),
            ["oewn", "mcr-spa"]
        );
        assert_eq!(
            rows(&conn, "SELECT value FROM meta WHERE key = 'license'"),
            [LICENSE]
        );
        assert_eq!(
            rows(&conn, "SELECT value FROM meta WHERE key = 'notice'"),
            [notice(false)]
        );
        assert!(!notice(false).contains("Wikidata"));
        assert!(notice(true).contains("CC0 1.0"));
        assert_eq!(report.get("wikidata: rows read"), None);
        assert_eq!(
            rows(
                &conn,
                "SELECT ifnull(s.gloss, '-') FROM sense s JOIN lexeme l ON l.id = s.lexeme_id \
                 WHERE l.lemma = 'perro' AND s.concept_id = 'i3'"
            ),
            ["-"]
        );
    }

    fn count(conn: &Connection, query: &str) -> i64 {
        conn.query_row(query, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn exception_forms_get_features_from_their_ending() {
        for (pos, form, features) in [
            ("n", "geese", "pl"),
            ("v", "went", "past"),
            ("v", "gone", "past"),
            ("v", "running", "ger"),
            ("v", "is", "pres,3sg"),
            ("v", "are", "pres"),
            ("adj", "better", "cmp"),
            ("adj", "worst", "sup"),
            ("adj", "worse", "cmp"),
        ] {
            assert_eq!(form_features(pos, form), features, "{form}");
        }
    }

    #[test]
    fn the_same_sources_build_the_same_bytes() {
        let dir = std::env::temp_dir().join(format!("talos-wordnet-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.talospack"), dir.join("b.talospack"));
        write_pack(&sample().0, &a).unwrap();
        write_pack(&sample().0, &b).unwrap();
        assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap());
        let checked = talos_core::knowledge::pack::check(&a).unwrap();
        assert!(checked.problems.is_empty(), "{:?}", checked.problems);
        assert_eq!(checked.outside_concepts, 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_or_unknown_sources_are_refused() {
        let dir = std::env::temp_dir().join(format!("talos-wordnet-src-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("en")).unwrap();
        let off = WikidataSource::Off;
        let error = build(&dir, &dir.join("x.talospack"), false, &off).unwrap_err();
        assert!(error.contains(LMF), "{error}");
        std::fs::write(dir.join("en").join(LMF), b"not the file").unwrap();
        std::fs::write(dir.join(ILI_MAP), b"").unwrap();
        std::fs::write(dir.join(SPANISH), b"").unwrap();
        let error = build(&dir, &dir.join("x.talospack"), false, &off).unwrap_err();
        assert!(error.contains("refusing an unknown file"), "{error}");
        // With Wikidata the 3.1 map is needed too, and the error says how to
        // do without; the snapshot is as well.
        let on = WikidataSource::Snapshot(dir.join("absent.tsv"));
        let error = build(&dir, &dir.join("x.talospack"), true, &on).unwrap_err();
        assert!(
            error.contains(ILI_MAP_31) && error.contains("--no-wikidata"),
            "{error}"
        );
        std::fs::write(dir.join(ILI_MAP_31), b"").unwrap();
        let error = build(&dir, &dir.join("x.talospack"), true, &on).unwrap_err();
        assert!(
            error.contains("Wikidata snapshot") && error.contains("--no-wikidata"),
            "{error}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
