//! `talos-pack kobo`: a dictionary in the format e-readers use (Kobo's
//! `dicthtml`) made into a dictionary pack.
//!
//! The file is a ZIP of gzipped HTML fragments, one per word prefix, each a
//! run of entries:
//!
//! ```text
//! <w><p><a name="perro" /><b>perro</b> [ˈpero]<br/><br/>
//!   <b>Sustantivo</b><ol><li>Variedad doméstica del lobo…</li>
//!     <ol style="…"><li><b>Sinónimos:</b> chucho, can.</li></ol>
//!     <li>…</li></ol>
//!   <b>Adjetivo</b><ol><li>Desdichado…</li></ol>
//!   <p>etymology</p></p>
//!   <var><variant name="perra"/><variant name="perros"/></var></w>
//! ```
//!
//! A pack keeps, for each headword, one lexeme per part of speech with its
//! numbered definitions as senses, and the inflected forms as `form` rows.
//! The sub-lists (synonyms), the pronunciation and the etymology are left
//! out. A form listed by an entry of a different number of words is dropped:
//! a locution lists every form of its verb ("fui" under "irse al mazo"),
//! which would send a verb form to the locution.
//!
//! Talos reads the pack with SQLite alone: nothing of the ZIP, gzip or HTML
//! reaches the binary. The content is whoever's dictionary it is; its name
//! and licence are given on the command line and written into the pack.
//! The same file always gives the same bytes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::Path;

use flate2::read::{DeflateDecoder, GzDecoder};
use rusqlite::{Connection, params};
use sha2::{Digest, Sha256};
use talos_core::knowledge::key;
use talos_core::knowledge::schema::{self, DbKind};

/// What the command line says about the dictionary.
pub(crate) struct Options {
    /// `es` or `en`: the language of its headwords and definitions.
    pub(crate) lang: String,
    /// The name answers cite ("Wikcionario").
    pub(crate) name: String,
    /// Its licence.
    pub(crate) license: String,
    /// Where it comes from.
    pub(crate) url: Option<String>,
}

/// One headword as read.
#[derive(Debug, Default, PartialEq, Eq)]
struct Entry {
    head: String,
    /// (part of speech, definitions), in the order written.
    sections: Vec<(String, Vec<String>)>,
    variants: Vec<String>,
}

fn le(bytes: &[u8], at: usize, len: usize) -> Result<usize, String> {
    let slice = bytes.get(at..at + len).ok_or("the ZIP file is cut short")?;
    Ok(slice
        .iter()
        .rev()
        .fold(0_usize, |n, b| (n << 8) | usize::from(*b)))
}

/// The files of a ZIP archive, by name, uncompressed. Reads the central
/// directory; stored and deflated entries only, no ZIP64 (a dictionary is
/// a few megabytes).
fn unzip(bytes: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    const END: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const CENTRAL: [u8; 4] = [0x50, 0x4b, 0x01, 0x02];
    let end = bytes
        .windows(4)
        .rposition(|w| w == END)
        .ok_or("not a ZIP file")?;
    let count = le(bytes, end + 10, 2)?;
    let mut at = le(bytes, end + 16, 4)?;
    let mut files = BTreeMap::new();
    for _ in 0..count {
        if bytes.get(at..at + 4) != Some(&CENTRAL) {
            return Err("the ZIP directory is damaged".to_owned());
        }
        let method = le(bytes, at + 10, 2)?;
        let packed = le(bytes, at + 20, 4)?;
        let size = le(bytes, at + 24, 4)?;
        let (name_len, extra, comment) = (
            le(bytes, at + 28, 2)?,
            le(bytes, at + 30, 2)?,
            le(bytes, at + 32, 2)?,
        );
        let local = le(bytes, at + 42, 4)?;
        let name = bytes
            .get(at + 46..at + 46 + name_len)
            .ok_or("the ZIP file is cut short")?;
        let name = String::from_utf8_lossy(name).into_owned();
        at += 46 + name_len + extra + comment;
        let start = local + 30 + le(bytes, local + 26, 2)? + le(bytes, local + 28, 2)?;
        let data = bytes
            .get(start..start + packed)
            .ok_or("the ZIP file is cut short")?;
        let content = match method {
            0 => data.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(size);
                DeflateDecoder::new(data)
                    .read_to_end(&mut out)
                    .map_err(|e| format!("{name}: {e}"))?;
                out
            }
            other => return Err(format!("{name}: ZIP compression method {other}")),
        };
        files.insert(name, content);
    }
    Ok(files)
}

/// A fragment's text: gunzipped when it is gzip, as it is otherwise.
fn text_of(name: &str, data: &[u8]) -> Result<String, String> {
    let bytes = if data.starts_with(&[0x1f, 0x8b]) {
        let mut out = Vec::new();
        GzDecoder::new(data)
            .read_to_end(&mut out)
            .map_err(|e| format!("{name}: {e}"))?;
        out
    } else {
        data.to_vec()
    };
    String::from_utf8(bytes).map_err(|_| format!("{name}: not UTF-8"))
}

/// Text with its character references resolved and its spaces collapsed.
fn clean(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let reference = rest[1..]
            .find(';')
            .filter(|end| *end <= 8)
            .map(|end| &rest[1..=end]);
        let resolved = reference.and_then(|r| match r {
            "nbsp" => Some(' '),
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => {
                let code = r.strip_prefix('#')?;
                let n = match code.strip_prefix(['x', 'X']) {
                    Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                    None => code.parse().ok()?,
                };
                char::from_u32(n)
            }
        });
        if let (Some(c), Some(r)) = (resolved, reference) {
            out.push(c);
            rest = &rest[r.len() + 2..];
        } else {
            out.push('&');
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn attribute(tag: &str) -> Option<String> {
    let value = tag.split_once("name=\"")?.1;
    Some(clean(value.split_once('"')?.0))
}

/// Reads one entry: what is between `<w>` and `</w>`. `None` when it has no
/// headword.
fn entry(block: &str) -> Option<Entry> {
    let mut e = Entry::default();
    let mut head = None;
    // Depth of <ol>: definitions are the items of a list at depth 1.
    let mut depth = 0_usize;
    // The last bold text at depth 0: a part of speech if a list follows.
    let mut bold: Option<String> = None;
    let mut in_bold = false;
    let mut item: Option<String> = None;
    // Inside <sub> or <sup>: cross-reference numbers, not text.
    let mut small = false;
    let mut rest = block;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix('<') {
            let (tag, tail) = after.split_once('>')?;
            rest = tail;
            let closing = tag.starts_with('/');
            let name = tag
                .trim_start_matches('/')
                .split(|c: char| c.is_whitespace() || c == '/')
                .next()
                .unwrap_or_default();
            match (name, closing) {
                ("a", false) if head.is_none() => head = attribute(tag),
                ("variant", false) => e.variants.extend(attribute(tag)),
                ("b", false) if depth == 0 => {
                    in_bold = true;
                    bold = Some(String::new());
                }
                ("b", true) => in_bold = false,
                ("ol", false) => {
                    if depth == 0 {
                        let pos = bold.take().map_or_default(|b| clean(&b));
                        e.sections.push((pos, Vec::new()));
                    }
                    depth += 1;
                }
                ("ol", true) => depth = depth.saturating_sub(1),
                ("li", false) if depth == 1 => item = Some(String::new()),
                ("li", true) if depth == 1 => {
                    if let (Some(text), Some(section)) = (item.take(), e.sections.last_mut()) {
                        let text = clean(&text);
                        if !text.is_empty() {
                            section.1.push(text);
                        }
                    }
                }
                ("sub" | "sup", _) => small = !closing,
                ("br", _) => {}
                _ if depth == 0 && !in_bold => bold = None,
                _ => {}
            }
        } else {
            let end = rest.find('<').unwrap_or(rest.len());
            let text = &rest[..end];
            rest = &rest[end..];
            if small {
                continue;
            }
            if depth == 1
                && let Some(item) = item.as_mut()
            {
                item.push_str(text);
            } else if depth == 0 {
                if in_bold {
                    if let Some(bold) = bold.as_mut() {
                        bold.push_str(text);
                    }
                } else if !text.trim().is_empty() {
                    bold = None;
                }
            }
        }
    }
    e.head = head.filter(|h| !h.is_empty())?;
    e.sections.retain(|(_, senses)| !senses.is_empty());
    Some(e)
}

/// The entries of a fragment, in order.
fn entries(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<w>") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("</w>") else { break };
        out.extend(entry(&after[..end]));
        rest = &after[end + 4..];
    }
    out
}

fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// What was read and written, for the person building.
#[derive(Debug, Default)]
pub(crate) struct Report {
    lines: Vec<String>,
}

impl Report {
    fn count(&mut self, what: &str, n: usize) {
        self.lines.push(format!("{n:>9}  {what}"));
    }

    pub(crate) fn text(&self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

fn sql(e: &rusqlite::Error) -> String {
    e.to_string()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// Builds the pack `out` from the dictionary file `file`.
///
/// # Errors
///
/// A message for a person when the file cannot be read, is not such a
/// dictionary, or a statement fails.
#[allow(clippy::too_many_lines, reason = "the build, step by step")]
pub(crate) fn build(file: &Path, out: &Path, options: &Options) -> Result<Report, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let sum = hex(&Sha256::digest(&bytes));
    let files = unzip(&bytes).map_err(|e| format!("{}: {e}", file.display()))?;
    let file_name = file.file_name().map_or_else(
        || "dictionary".to_owned(),
        |n| n.to_string_lossy().into_owned(),
    );
    let version = files
        .get("___snapshot.txt")
        .map(|v| String::from_utf8_lossy(v).trim().to_owned())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| sum[..12].to_owned());
    let id = format!("dictionary-{}", options.lang);
    let mut report = Report::default();
    report.lines.push(format!("source: {file_name} {sum}"));

    let mut conn = Connection::open_in_memory().map_err(|e| sql(&e))?;
    let tx = conn.transaction().map_err(|e| sql(&e))?;
    schema::migrate(&tx, "main", DbKind::Pack).map_err(|e| e.to_string())?;
    let from = options
        .url
        .as_ref()
        .map_or_else(String::new, |u| format!(" ({u})"));
    let notice = format!(
        "Definitions and inflected forms from {name}{from}, licence {license}; converted \
         without change of wording from the file {file_name} (SHA-256 {sum}). Synonym lists, \
         pronunciations and etymologies were left out.",
        name = options.name,
        license = options.license,
    );
    let title = format!("{} ({})", options.name, options.lang);
    for (k, v) in [
        ("id", id.as_str()),
        ("version", version.as_str()),
        ("title", title.as_str()),
        ("languages", options.lang.as_str()),
        ("license", options.license.as_str()),
        ("notice", notice.as_str()),
        ("role", "dictionary"),
        ("source_files", &format!("{file_name} {sum}")),
    ] {
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            [k, v],
        )
        .map_err(|e| sql(&e))?;
    }
    tx.execute(
        "INSERT INTO source (id, kind, name, license, url, added_at) \
         VALUES (1, 'import', ?1, ?2, ?3, 0)",
        params![options.name, options.license, options.url],
    )
    .map_err(|e| sql(&e))?;

    let (mut heads, mut empty, mut dropped) = (0_usize, 0_usize, 0_usize);
    let (mut n_senses, mut n_forms) = (0_usize, 0_usize);
    let mut by_pos: BTreeMap<String, usize> = BTreeMap::new();
    {
        let mut lexeme = tx
            .prepare("INSERT INTO lexeme (id, lang, lemma, pos) VALUES (?1, ?2, ?3, ?4)")
            .map_err(|e| sql(&e))?;
        let mut form = tx
            .prepare("INSERT INTO form (lexeme_id, form, features) VALUES (?1, ?2, ?3)")
            .map_err(|e| sql(&e))?;
        let mut concept = tx
            .prepare("INSERT INTO concept (id, kind) VALUES (?1, 'sense')")
            .map_err(|e| sql(&e))?;
        let mut sense = tx
            .prepare(
                "INSERT INTO sense (lexeme_id, concept_id, gloss, rank, source_id) \
                 VALUES (?1, ?2, ?3, ?4, 1)",
            )
            .map_err(|e| sql(&e))?;
        // (lemma, part of speech) → lexeme id and how many senses it has.
        let mut lexemes: HashMap<(String, String), (i64, i64)> = HashMap::new();
        let mut forms: HashSet<(i64, String)> = HashSet::new();
        let mut next = 0_i64;
        let mut next_sense = 0_i64;
        for (name, data) in &files {
            if !Path::new(name)
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("html"))
            {
                continue;
            }
            for e in entries(&text_of(name, data)?) {
                if e.sections.is_empty() {
                    empty += 1;
                    continue;
                }
                heads += 1;
                let mut first = None;
                for (pos, senses) in &e.sections {
                    let pos = pos.to_lowercase();
                    let slot = lexemes.entry((e.head.clone(), pos.clone()));
                    let (lexeme_id, count) = match slot {
                        std::collections::hash_map::Entry::Occupied(o) => o.into_mut(),
                        std::collections::hash_map::Entry::Vacant(v) => {
                            next += 1;
                            let stored = (!pos.is_empty()).then_some(pos.as_str());
                            lexeme
                                .execute(params![next, options.lang, e.head, stored])
                                .map_err(|e| sql(&e))?;
                            *by_pos.entry(pos.clone()).or_default() += 1;
                            let lemma = key(&e.head);
                            if !lemma.is_empty() && forms.insert((next, lemma.clone())) {
                                form.execute(params![next, lemma, None::<&str>])
                                    .map_err(|e| sql(&e))?;
                            }
                            v.insert((next, 0))
                        }
                    };
                    first.get_or_insert(*lexeme_id);
                    for gloss in senses {
                        next_sense += 1;
                        // Short: the id is stored three times per definition.
                        let concept_id = format!("d-{}:{next_sense:x}", options.lang);
                        concept.execute([&concept_id]).map_err(|e| sql(&e))?;
                        sense
                            .execute(params![*lexeme_id, concept_id, gloss, *count])
                            .map_err(|e| sql(&e))?;
                        *count += 1;
                        n_senses += 1;
                    }
                }
                let Some(first) = first else { continue };
                let size = words(&e.head);
                for variant in &e.variants {
                    let v = key(variant);
                    if v.is_empty() || words(&v) != size {
                        dropped += 1;
                        continue;
                    }
                    if forms.insert((first, v.clone())) {
                        form.execute(params![first, v, "form"])
                            .map_err(|e| sql(&e))?;
                        n_forms += 1;
                    }
                }
            }
        }
    }
    if heads == 0 {
        return Err(format!(
            "{}: no dictionary entries found (is it a Kobo dicthtml file?)",
            file.display()
        ));
    }
    tx.commit().map_err(|e| sql(&e))?;
    report.count("headwords", heads);
    report.count("entries without a definition, left out", empty);
    report.count("definitions", n_senses);
    report.count("inflected forms", n_forms);
    report.count(
        "forms dropped (listed by an entry of another number of words)",
        dropped,
    );
    let mut pos: Vec<(&String, &usize)> = by_pos.iter().collect();
    pos.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (label, n) in pos.iter().take(12) {
        report.count(&format!("lexemes: {label}"), **n);
    }
    report.count("parts of speech in all", pos.len());

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
    let notice_path = out.with_extension("NOTICE");
    std::fs::write(&notice_path, format!("{notice}\n"))
        .map_err(|e| format!("{}: {e}", notice_path.display()))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERRO: &str = "<p><a name=\"perro\" /><b>perro</b> [ˈpero]<br/><br/>\
        <b>Sustantivo</b><ol><li>Mamíferos: (<i>Canis</i> <i>lupus</i>) Variedad&nbsp;doméstica \
        del lobo.</li><ol style=\"list-style-type:lower-alpha\"><li><b>Sinónimos:</b> chucho, \
        can.</li></ol><li>Ir<sub>1</sub> a la pinza &amp; volver.</li></ol>\
        <b>Adjetivo</b><ol><li>Desdichado, indigno.</li></ol><p>De origen <b>incierto</b>.</p>\
        </p><var><variant name=\"perra\"/><variant name=\"perros\"/></var>";

    #[test]
    fn an_entry_gives_its_sections_definitions_and_forms() {
        let e = entry(PERRO).unwrap();
        assert_eq!(e.head, "perro");
        assert_eq!(
            e.sections,
            vec![
                (
                    "Sustantivo".to_owned(),
                    vec![
                        "Mamíferos: (Canis lupus) Variedad doméstica del lobo.".to_owned(),
                        "Ir a la pinza & volver.".to_owned()
                    ]
                ),
                (
                    "Adjetivo".to_owned(),
                    vec!["Desdichado, indigno.".to_owned()]
                ),
            ]
        );
        assert_eq!(e.variants, ["perra", "perros"]);
    }

    #[test]
    fn fragments_split_into_entries() {
        let text = format!("<html><w>{PERRO}</w><w><p><a name=\"x\"/></p></w></html>");
        let all = entries(&text);
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].sections.len(), 0);
    }

    /// A ZIP holding `files`, stored (not compressed).
    fn zip(files: &[(&str, &str)]) -> Vec<u8> {
        let (mut out, mut directory) = (Vec::new(), Vec::new());
        let n16 = |n: usize| u16::try_from(n).unwrap().to_le_bytes();
        let n32 = |n: usize| u32::try_from(n).unwrap().to_le_bytes();
        for (name, content) in files {
            let offset = out.len();
            // Version, flags, method, time, date, CRC: none checked.
            let mut common = vec![0_u8; 14];
            common.extend(n32(content.len()));
            common.extend(n32(content.len()));
            common.extend(n16(name.len()));
            common.extend(n16(0));
            out.extend([0x50, 0x4b, 0x03, 0x04]);
            out.extend(&common);
            out.extend(name.as_bytes());
            out.extend(content.as_bytes());
            directory.extend([0x50, 0x4b, 0x01, 0x02, 0, 0]);
            directory.extend(&common);
            directory.extend([0_u8; 10]);
            directory.extend(n32(offset));
            directory.extend(name.as_bytes());
        }
        let start = out.len();
        out.extend(&directory);
        out.extend([0x50, 0x4b, 0x05, 0x06, 0, 0, 0, 0]);
        out.extend(n16(files.len()));
        out.extend(n16(files.len()));
        out.extend(n32(directory.len()));
        out.extend(n32(start));
        out.extend(n16(0));
        out
    }

    #[test]
    fn a_dictionary_file_becomes_a_pack_define_can_ask() {
        use talos_core::Lang;
        use talos_core::knowledge::Knowledge;

        let ir = "<w><p><a name=\"ir\"/><b>ir</b><br/><b>Verbo</b><ol><li>Desplazarse.</li>\
                  </ol></p><var><variant name=\"fui\"/></var></w>\
                  <w><p><a name=\"irse al mazo\"/><b>Locución</b><ol><li>Desistir.</li></ol></p>\
                  <var><variant name=\"fui\"/></var></w>\
                  <w><p><a name=\"irse\"/><b>Verbo</b><ol><li>Marcharse.</li></ol></p>\
                  <var><variant name=\"fui\"/></var></w>";
        let dir = std::env::temp_dir().join(format!("talos-kobo-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (file, out) = (dir.join("d.zip"), dir.join("d.talospack"));
        let fragment = format!("<html><w>{PERRO}</w></html>");
        std::fs::write(
            &file,
            zip(&[
                ("___snapshot.txt", "20260101"),
                ("pe.html", &fragment),
                ("ir.html", ir),
            ]),
        )
        .unwrap();
        let options = Options {
            lang: "es".to_owned(),
            name: "Prueba".to_owned(),
            license: "CC0-1.0".to_owned(),
            url: None,
        };
        build(&file, &out, &options).unwrap();
        let first = std::fs::read(&out).unwrap();
        build(&file, &out, &options).unwrap();
        assert_eq!(
            first,
            std::fs::read(&out).unwrap(),
            "the same file gives the same bytes"
        );

        let checked = talos_core::knowledge::pack::check(&out).unwrap();
        assert_eq!(checked.problems, Vec::<String>::new());
        assert_eq!(checked.info.version, "20260101");
        let mut k = Knowledge::empty().unwrap();
        assert!(k.attach(&out), "{:?}", k.refused());

        let perro = k.entries("Perro", Lang::Es).unwrap();
        assert_eq!(perro.len(), 1);
        assert!(!perro[0].inflected);
        assert_eq!(perro[0].source.name, "Prueba");
        let pos: Vec<_> = perro[0].sections.iter().map(|s| s.pos.as_deref()).collect();
        assert_eq!(pos, [Some("sustantivo"), Some("adjetivo")]);
        assert_eq!(perro[0].sections[0].definitions.len(), 2);

        let perros = k.entries("perros", Lang::Es).unwrap();
        assert!(perros[0].inflected);
        assert_eq!(perros[0].lemma, "perro");
        // The locution's copy of the verb's forms is dropped, and the
        // pronominal verb is one headword with its verb.
        let fui: Vec<String> = k
            .entries("fui", Lang::Es)
            .unwrap()
            .into_iter()
            .map(|e| e.lemma)
            .collect();
        assert_eq!(fui, ["ir"]);
        assert_eq!(k.entries("perro", Lang::En).unwrap().len(), 0);
        // A dictionary's rows are not concepts: the concept queries do not
        // see them.
        assert!(k.lookup("perro", Lang::Es).unwrap().is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn references_resolve() {
        assert_eq!(
            clean("a&nbsp;b &#233; &#xE9; &amp; R&D &zzz;"),
            "a b é é & R&D &zzz;"
        );
    }
}
