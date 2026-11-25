//! Pattern lists, cheap to hold (docs/DEVELOPMENT.md, "Footprint").
//!
//! Every pattern of the manifests and of the reflector scripts is matched
//! against the **match form** of a message ([`normalise`](crate::normalise)).
//! Compiled with the `regex` crate as written, one `RegexSet` per list plus
//! one `Regex` per pattern, they cost ~16 MB of resident memory and ~85 ms
//! at start-up, most of it for the Unicode classes: each `\w` is a UTF-8
//! automaton of ~320 states, repeated in every `(?:\w+ ){0,3}`. A
//! [`Patterns`] list keeps that off the common path without changing what
//! any pattern matches:
//!
//! - **Two forms of each pattern.** Most messages hold no word or space
//!   character outside ASCII once folded (accents are gone, `ñ` is `n`;
//!   `¿`, `¡`, `€` are neither). On such a text a character class means the
//!   same as its ASCII part plus what it holds of the non-ASCII characters
//!   that are not word or space characters, so the **narrow form** rewrites
//!   each class that way (`\w` → `[0-9A-Z_a-z]`, `[^\d]` → `[^0-9]`,
//!   `[¿¡\s]` → `[\t-\r ¡¿]`) and matches exactly as the pattern does. A
//!   text with such a character (`ß`, `ø`, `µ`, Greek, Cyrillic, CJK) is
//!   matched with the patterns as written, compiled when the first one
//!   arrives.
//! - **Built when first needed.** Patterns are parsed, and so validated, up
//!   front; automata are built on first use, per list and form, and a single
//!   pattern's own automaton (to read its captures) only when that pattern
//!   is read.
//! - **Small automata, no kept caches.** A list is matched as a few small
//!   multi-pattern automata: building one large one needs a transient peak
//!   several times its final size, which the allocator keeps. They are the
//!   `PikeVM`s the `regex` crate itself would run (same parser, same
//!   compiler, same leftmost-first and overlapping semantics), driven
//!   directly so that the scratch space of a search is freed after it
//!   instead of being kept per automaton.

use std::ops::Range;
use std::sync::OnceLock;

use regex_automata::nfa::thompson::pikevm::PikeVM;
use regex_automata::nfa::thompson::{self, NFA};
use regex_automata::{Input, MatchKind, PatternSet};
use regex_syntax::hir::{Class, Hir, HirKind, Look};

/// Patterns per multi-pattern automaton: small ones keep the build-time
/// peak low and cost little more to search than one large one.
const CHUNK: usize = 8;

/// The largest automaton built, as the `regex` crate's default.
const SIZE_LIMIT: usize = 10 << 20;

/// A list of patterns, matched together or one by one.
#[derive(Debug)]
pub(crate) struct Patterns {
    entries: Vec<Entry>,
    /// Whether any pattern has a narrow form distinct from its source.
    narrowed: bool,
    /// The multi-pattern automata, by [`Form`].
    sets: [OnceLock<Vec<Set>>; 2],
}

/// Up to [`CHUNK`] patterns matched together.
#[derive(Debug)]
struct Set {
    vm: PikeVM,
    lengths: Lengths,
}

/// The lengths of text, in bytes, on which a pattern can match: no
/// shorter than `min` and, for a pattern anchored at both ends, no longer
/// than `max`. A search on any other text is skipped, as the `regex` crate
/// skips it.
#[derive(Debug, Clone, Copy)]
struct Lengths {
    min: usize,
    max: Option<usize>,
}

impl Lengths {
    fn of(hir: &Hir) -> Self {
        let props = hir.properties();
        let anchored = props.look_set_prefix().contains(Look::Start)
            && props.look_set_suffix().contains(Look::End);
        Self {
            // A pattern that can match nothing has no minimum.
            min: props.minimum_len().unwrap_or(usize::MAX),
            max: props.maximum_len().filter(|_| anchored),
        }
    }

    /// Lengths that cover all of `lengths`.
    fn union<'a>(lengths: impl IntoIterator<Item = &'a Lengths>) -> Self {
        lengths.into_iter().fold(
            Self {
                min: usize::MAX,
                max: Some(0),
            },
            |all, l| Self {
                min: all.min.min(l.min),
                max: all.max.zip(l.max).map(|(a, b)| a.max(b)),
            },
        )
    }

    fn admit(self, text: &str) -> bool {
        text.len() >= self.min && self.max.is_none_or(|max| text.len() <= max)
    }
}

#[derive(Debug)]
struct Entry {
    /// The pattern as written.
    source: String,
    /// The narrow form, when it differs from the source.
    narrow: Option<String>,
    /// Its named captures, in order.
    names: Vec<String>,
    lengths: Lengths,
    /// Its own automaton, by [`Form`], built when first read.
    single: [OnceLock<Option<PikeVM>>; 2],
}

/// Which compiled form serves a text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    Narrow = 0,
    Written = 1,
}

impl Patterns {
    /// Parses every pattern; nothing is compiled yet.
    ///
    /// # Errors
    ///
    /// The index of the first pattern that does not parse, and why.
    pub(crate) fn new<'a>(
        sources: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, (usize, String)> {
        let mut parser = regex_syntax::Parser::new();
        let entries: Vec<Entry> = sources
            .into_iter()
            .enumerate()
            .map(|(index, source)| {
                let hir = parser.parse(source).map_err(|e| (index, e.to_string()))?;
                let mut names = Vec::new();
                capture_names(&hir, &mut names);
                Ok(Entry {
                    source: source.to_owned(),
                    narrow: narrow(source),
                    names,
                    lengths: Lengths::of(&hir),
                    single: [OnceLock::new(), OnceLock::new()],
                })
            })
            .collect::<Result<_, _>>()?;
        Ok(Self::from_entries(entries))
    }

    /// The lists one after the other, as one.
    pub(crate) fn concat(lists: Vec<Patterns>) -> Self {
        Self::from_entries(lists.into_iter().flat_map(|l| l.entries).collect())
    }

    fn from_entries(entries: Vec<Entry>) -> Self {
        Self {
            narrowed: entries.iter().any(|e| e.narrow.is_some()),
            entries,
            sets: [OnceLock::new(), OnceLock::new()],
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Which forms' multi-pattern automata exist: narrow, as written.
    #[cfg(test)]
    pub(crate) fn built(&self) -> [bool; 2] {
        self.sets.each_ref().map(|s| s.get().is_some())
    }

    /// Pattern `i` as written.
    pub(crate) fn source(&self, i: usize) -> &str {
        &self.entries[i].source
    }

    /// The named captures of pattern `i`, in order.
    pub(crate) fn names(&self, i: usize) -> &[String] {
        &self.entries[i].names
    }

    /// The patterns that match `text` anywhere, in ascending order: what
    /// `RegexSet::matches` reports.
    pub(crate) fn matches(&self, text: &str) -> Vec<usize> {
        self.matches_in(text, Form::of(text, self.narrowed))
    }

    fn matches_in(&self, text: &str, form: Form) -> Vec<usize> {
        let sets = self.sets[form as usize].get_or_init(|| {
            self.entries
                .chunks(CHUNK)
                .map(|chunk| {
                    let sources: Vec<&str> = chunk.iter().map(|e| e.text(form)).collect();
                    Some(Set {
                        vm: build(&sources, MatchKind::All, thompson::WhichCaptures::None)?,
                        lengths: Lengths::union(chunk.iter().map(|e| &e.lengths)),
                    })
                })
                .collect::<Option<_>>()
                // Not reached: every pattern parsed. Were it, nothing
                // would match.
                .unwrap_or_default()
        });
        // One scratch space for the whole search, freed when it is done.
        let mut cache = None;
        let mut set = PatternSet::new(CHUNK);
        let input = Input::new(text);
        let mut found = Vec::new();
        for (n, Set { vm, lengths }) in sets.iter().enumerate() {
            if !lengths.admit(text) {
                continue;
            }
            let cache = cache.get_or_insert_with(|| vm.create_cache());
            vm.reset_cache(cache);
            set.clear();
            vm.which_overlapping_matches(cache, &input, &mut set);
            found.extend(set.iter().map(|p| n * CHUNK + p.as_usize()));
        }
        found
    }

    /// The automaton of pattern `i` alone that serves `text`.
    fn single(&self, i: usize, text: &str) -> Option<&PikeVM> {
        self.single_in(i, text, Form::of(text, self.entries[i].narrow.is_some()))
    }

    fn single_in(&self, i: usize, text: &str, form: Form) -> Option<&PikeVM> {
        let entry = &self.entries[i];
        if !entry.lengths.admit(text) {
            return None;
        }
        entry.single[form as usize]
            .get_or_init(|| {
                build(
                    &[entry.text(form)],
                    MatchKind::LeftmostFirst,
                    thompson::WhichCaptures::All,
                )
            })
            .as_ref()
    }

    /// Whether pattern `i` matches `text`.
    pub(crate) fn is_match(&self, i: usize, text: &str) -> bool {
        self.single(i, text)
            .is_some_and(|vm| vm.is_match(&mut vm.create_cache(), text))
    }

    /// The named captures of pattern `i` in `text`, as byte ranges, when
    /// the pattern matches; a capture that took no part is left out.
    pub(crate) fn captures(&self, i: usize, text: &str) -> Option<Vec<(&str, Range<usize>)>> {
        self.captures_in(i, text, Form::of(text, self.entries[i].narrow.is_some()))
    }

    fn captures_in(&self, i: usize, text: &str, form: Form) -> Option<Vec<(&str, Range<usize>)>> {
        let vm = self.single_in(i, text, form)?;
        let mut caps = vm.create_captures();
        vm.captures(&mut vm.create_cache(), text, &mut caps);
        caps.is_match().then(|| {
            self.names(i)
                .iter()
                .filter_map(|name| {
                    caps.get_group_by_name(name)
                        .map(|span| (name.as_str(), span.range()))
                })
                .collect()
        })
    }

    /// `text` with every match of pattern `i` replaced by as many spaces as
    /// it has bytes.
    pub(crate) fn blank(&self, i: usize, text: &str) -> String {
        let Some(vm) = self.single(i, text) else {
            return text.to_owned();
        };
        let mut out = String::with_capacity(text.len());
        let mut last = 0;
        for m in vm.find_iter(&mut vm.create_cache(), text) {
            out.push_str(&text[last..m.start()]);
            out.extend(std::iter::repeat_n(' ', m.len()));
            last = m.end();
        }
        out.push_str(&text[last..]);
        out
    }
}

/// An automaton for `sources`, configured as the `regex` crate configures
/// its own: a `RegexSet` reports every pattern that matches
/// (`MatchKind::All`, no captures), a `Regex` the leftmost-first match.
fn build(sources: &[&str], kind: MatchKind, captures: thompson::WhichCaptures) -> Option<PikeVM> {
    let nfa = NFA::compiler()
        .configure(
            thompson::Config::new()
                .which_captures(captures)
                .nfa_size_limit(Some(SIZE_LIMIT)),
        )
        .build_many(sources)
        .ok()?;
    PikeVM::builder()
        .configure(PikeVM::config().match_kind(kind))
        .build_from_nfa(nfa)
        .ok()
}

impl Entry {
    fn text(&self, form: Form) -> &str {
        match (form, &self.narrow) {
            (Form::Narrow, Some(narrow)) => narrow,
            _ => &self.source,
        }
    }
}

impl Form {
    /// Narrow unless `text` holds a word or space character outside ASCII
    /// and the patterns have a narrow form at all.
    fn of(text: &str, narrowed: bool) -> Self {
        if narrowed && !text.is_ascii() && text.chars().any(is_wide) {
            Form::Written
        } else {
            Form::Narrow
        }
    }
}

/// Whether `c` is a word or space character outside ASCII: what `\w`,
/// `\d` and `\s` match beyond their ASCII part.
fn is_wide(c: char) -> bool {
    static WIDE: OnceLock<Vec<(char, char)>> = OnceLock::new();
    if c.is_ascii() {
        return false;
    }
    let ranges = WIDE.get_or_init(|| {
        match regex_syntax::Parser::new()
            .parse(r"[\w\s]")
            .map(Hir::into_kind)
        {
            Ok(HirKind::Class(Class::Unicode(class))) => class
                .ranges()
                .iter()
                .map(|r| (r.start(), r.end()))
                .collect(),
            // Not reached: the expression is fixed. Were it, every
            // character would count as wide and every text that is not
            // ASCII would be matched with the patterns as written.
            _ => vec![('\0', char::MAX)],
        }
    });
    let i = ranges.partition_point(|&(_, end)| end < c);
    ranges.get(i).is_some_and(|&(start, _)| start <= c)
}

/// The narrow form of a pattern, if it differs: `\w`, `\d`, `\s` and their
/// negations (`\W`, `\D`, `\S`) replaced by their ASCII classes. On a text
/// with no [wide](is_wide) character it matches exactly as the pattern:
/// such a text holds none of the characters the replacement leaves out.
/// Every other class is kept; `.`, `[^…]` and `[¿¡]` are cheap as written.
/// The replacements are bracketed classes, valid inside a class too
/// (`[¿¡\s]` → `[¿¡[\t-\r\x20]]`), and avoid a literal space so that they
/// mean the same under the `x` flag.
fn narrow(source: &str) -> Option<String> {
    let mut out = String::with_capacity(source.len() + 32);
    let mut changed = false;
    let mut chars = source.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(next) = chars.next() else {
            out.push(c);
            break;
        };
        let ascii = match next {
            'w' => "[0-9A-Z_a-z]",
            'W' => "[^0-9A-Z_a-z]",
            'd' => "[0-9]",
            'D' => "[^0-9]",
            's' => r"[\t-\r\x20]",
            'S' => r"[^\t-\r\x20]",
            _ => {
                out.push(c);
                out.push(next);
                continue;
            }
        };
        out.push_str(ascii);
        changed = true;
    }
    changed.then_some(out)
}

/// The named captures of `hir`, in the order of their groups.
fn capture_names(hir: &Hir, names: &mut Vec<String>) {
    match hir.kind() {
        HirKind::Capture(capture) => {
            if let Some(name) = &capture.name {
                names.push(name.to_string());
            }
            capture_names(&capture.sub, names);
        }
        HirKind::Repetition(rep) => capture_names(&rep.sub, names),
        HirKind::Concat(subs) | HirKind::Alternation(subs) => {
            for sub in subs {
                capture_names(sub, names);
            }
        }
        HirKind::Empty | HirKind::Literal(_) | HirKind::Class(_) | HirKind::Look(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalise::normalise;
    use std::path::Path;

    #[test]
    fn the_narrow_form_replaces_only_the_perl_classes() {
        assert_eq!(narrow(r"^hola$"), None);
        assert_eq!(narrow(r"a\.b\\w"), None);
        assert_eq!(
            narrow(r"^[¿¡\s]*(?P<n>\d+)\w\b"),
            Some(r"^[¿¡[\t-\r\x20]]*(?P<n>[0-9]+)[0-9A-Z_a-z]\b".to_owned())
        );
        assert_eq!(
            narrow(r"[^\d]\W\S\D"),
            Some(r"[^[0-9]][^0-9A-Z_a-z][^\t-\r\x20][^0-9]".to_owned())
        );
    }

    #[test]
    fn a_text_is_narrow_unless_it_has_a_word_or_space_character_beyond_ascii() {
        for text in [
            "que hora es",
            "¿que hora es?",
            "¡hola!",
            "2 € y 3 £",
            "45°",
            "hola 😀",
        ] {
            assert_eq!(Form::of(text, true), Form::Narrow, "{text}");
        }
        for text in ["straße", "5 µm", "Ωmega", "москва", "咖啡", "٣", "a\u{a0}b"] {
            assert_eq!(Form::of(text, true), Form::Written, "{text}");
            assert_eq!(Form::of(text, false), Form::Narrow, "{text}");
        }
    }

    #[test]
    fn automata_are_built_when_first_needed_and_only_in_the_form_needed() {
        let list = Patterns::new([r"^(?P<w>\w+) es$", r"\d", "hola"]).unwrap();
        assert_eq!(list.built(), [false, false]);
        assert_eq!(list.matches("cine es"), [0]);
        assert_eq!(list.captures(0, "cine es"), Some(vec![("w", 0..4)]));
        assert_eq!(list.built(), [true, false]);
        assert!(list.entries[1].single.iter().all(|s| s.get().is_none()));
        // A wide text is matched as written.
        assert_eq!(list.matches("straße es"), [0]);
        assert_eq!(list.captures(0, "straße es"), Some(vec![("w", 0..7)]));
        assert_eq!(list.matches("٣"), [1]);
        assert_eq!(list.built(), [true, true]);
        assert_eq!(list.blank(1, "a1b22"), "a b  ");
        assert!(list.is_match(2, "di hola") && !list.is_match(2, "adios"));
    }

    /// Every built-in pattern: the manifests' and the reflector scripts'.
    fn builtin_patterns(root: &Path) -> Vec<String> {
        fn walk(value: &toml::Value, key: &str, out: &mut Vec<String>) {
            match value {
                toml::Value::Table(table) => {
                    for (k, v) in table {
                        walk(v, k, out);
                    }
                }
                toml::Value::Array(items) => items.iter().for_each(|v| walk(v, key, out)),
                toml::Value::String(s)
                    if ["regex", "patterns", "unless", "idioms"].contains(&key) =>
                {
                    out.push(s.clone());
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for text in [crate::reflector::SCRIPT_ES, crate::reflector::SCRIPT_EN]
            .map(str::to_owned)
            .into_iter()
            .chain(files(&root.join("skills"), "manifest.toml"))
        {
            walk(&toml::from_str(&text).unwrap(), "", &mut out);
        }
        out
    }

    /// The text of every file named `name` (or ending in `.txt` when
    /// `name` is empty) under `dir`.
    fn files(dir: &Path, name: &str) -> Vec<String> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(files(&path, name));
            } else if path.file_name().is_some_and(|n| n == name)
                || name.is_empty() && path.extension().is_some_and(|e| e == "txt")
            {
                out.push(std::fs::read_to_string(&path).unwrap());
            }
        }
        out
    }

    /// The narrow form must match exactly as the pattern as written on
    /// every narrow text: here, every user line of the golden files, every
    /// manifest example, and texts with non-ASCII characters, in their
    /// match form, whole and in pieces.
    #[test]
    fn the_narrow_form_matches_as_written_on_narrow_texts() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let patterns = builtin_patterns(&root);
        assert!(patterns.len() > 400, "{}", patterns.len());
        let list = Patterns::new(patterns.iter().map(String::as_str)).unwrap();
        let mut texts: Vec<String> = files(&root.join("golden"), "")
            .iter()
            .flat_map(|f| {
                f.lines()
                    .filter_map(|l| l.split_once("U: "))
                    .map(|(_, u)| u.to_owned())
            })
            .collect();
        for manifest in files(&root.join("skills"), "manifest.toml") {
            let manifest: toml::Value = toml::from_str(&manifest).unwrap();
            for intent in manifest["intent"].as_array().unwrap() {
                for examples in intent["examples"].as_table().unwrap().values() {
                    let examples = examples.as_array().unwrap();
                    texts.extend(examples.iter().map(|e| e.as_str().unwrap().to_owned()));
                }
            }
        }
        texts.extend(
            [
                "¿Qué hora es?",
                "¡Hola, Talos!",
                "cuánto es 2,5 € más 3 £",
                "45° en fahrenheit",
                "hola 😀",
                "elige entre café, té o agua",
                "creo que el cine es aburrido, ¿no?",
                "i think «movies» are boring",
                "x ÷ y",
                "-5 × 3",
                "½ de 10",
            ]
            .map(str::to_owned),
        );
        let mut narrow = std::collections::BTreeSet::new();
        for text in &texts {
            let folded = normalise(text).folded;
            let pieces = folded
                .split([',', '.', '?', '!'])
                .map(|p| p.trim().to_owned());
            narrow.extend(
                std::iter::once(folded.clone())
                    .chain(pieces)
                    .filter(|t| Form::of(t, true) == Form::Narrow),
            );
        }
        assert!(narrow.len() > 1000, "{}", narrow.len());
        for text in &narrow {
            let found = list.matches_in(text, Form::Narrow);
            assert_eq!(found, list.matches_in(text, Form::Written), "{text}");
            for &i in &found {
                assert_eq!(
                    list.captures_in(i, text, Form::Narrow),
                    list.captures_in(i, text, Form::Written),
                    "/{}/ on {text:?}",
                    list.source(i)
                );
            }
        }
    }
}
