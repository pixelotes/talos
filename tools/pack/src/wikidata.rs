//! The Wikidata snapshot (`knowledge/sources/wikidata-p8814-es.tsv`): the
//! Spanish description of every item that names a WordNet 3.1 synset
//! (property P8814), as exported by a SPARQL query. Three tab-separated
//! columns after a header line: the item as `<http://www.wikidata.org/entity/Q…>`,
//! the synset id as `"08978603-n"`, the description as `"…"@es`, with the
//! literal's backslash escapes (`\"`, `\\`, `\n`, `\t`, `\uXXXX`).
//!
//! The reader is strict: a line that is not a row is an error naming its
//! line, so a truncated or damaged export (a query that timed out writes its
//! error after the rows) is never taken for data.

use std::io::BufRead;

/// The header line of the export.
const HEADER: &str = "?i\t?s\t?d";
const ENTITY: &str = "<http://www.wikidata.org/entity/Q";

/// One row of the snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    /// The item's Q number.
    pub(crate) item: u64,
    /// The WordNet 3.1 synset id as written (`08978603-n`).
    pub(crate) synset: String,
    /// The description, unescaped, whitespace collapsed; may be empty.
    pub(crate) description: String,
}

/// The text of an RDF literal's quotes, with the escapes resolved.
fn unescape(text: &str) -> Option<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            '"' => out.push('"'),
            '\'' => out.push('\''),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            marker @ ('u' | 'U') => {
                let n = if marker == 'u' { 4 } else { 8 };
                let digits: String = chars.by_ref().take(n).collect();
                if digits.len() != n {
                    return None;
                }
                out.push(char::from_u32(u32::from_str_radix(&digits, 16).ok()?)?);
            }
            _ => return None,
        }
    }
    Some(out)
}

/// One data line; `None` when it is not a row.
fn row(line: &str) -> Option<Row> {
    let mut fields = line.split('\t');
    let (item, synset, description) = (fields.next()?, fields.next()?, fields.next()?);
    if fields.next().is_some() {
        return None;
    }
    let item = item
        .strip_prefix(ENTITY)?
        .strip_suffix('>')?
        .parse::<u64>()
        .ok()?;
    let synset = synset.strip_prefix('"')?.strip_suffix('"')?;
    let description = unescape(description.strip_prefix('"')?.strip_suffix("\"@es")?)?;
    Some(Row {
        item,
        synset: synset.to_owned(),
        description: description.split_whitespace().collect::<Vec<_>>().join(" "),
    })
}

/// Reads the snapshot: its rows in file order.
///
/// # Errors
///
/// A message naming the line when the header is missing or a line is not a
/// row.
pub(crate) fn read(input: impl BufRead) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", i + 1))?;
        if i == 0 {
            if line != HEADER {
                return Err(format!("line 1 is not the header `?i ?s ?d`: {line:?}"));
            }
            continue;
        }
        let shown: String = line.chars().take(80).collect();
        rows.push(row(&line).ok_or_else(|| {
            format!(
                "line {} is not a row of the export (a truncated or damaged file?): {shown:?}",
                i + 1
            )
        })?);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_read_with_their_escapes() {
        let text = "?i\t?s\t?d\n\
            <http://www.wikidata.org/entity/Q1013>\t\"08978603-n\"\t\"país  de África\"@es\n\
            <http://www.wikidata.org/entity/Q183424>\t\"09654317-n\"\t\"persona \\\"non grata\\\" \\u00e9\"@es\n";
        let rows = read(text.as_bytes()).unwrap();
        assert_eq!(
            rows,
            [
                Row {
                    item: 1013,
                    synset: "08978603-n".to_owned(),
                    description: "país de África".to_owned()
                },
                Row {
                    item: 183_424,
                    synset: "09654317-n".to_owned(),
                    description: "persona \"non grata\" é".to_owned()
                }
            ]
        );
    }

    #[test]
    fn a_damaged_export_is_refused_with_its_line() {
        let cut = "?i\t?s\t?d\n<http://www.wikidata.org/entity/Q1>\t\"1-n\"\t\"x\"@es\n\
                   <http://www.wiki";
        let error = read(cut.as_bytes()).unwrap_err();
        assert!(error.starts_with("line 3 is not a row"), "{error}");
        assert!(read("no header\n".as_bytes()).is_err());
        let other = "?i\t?s\t?d\n<http://www.wikidata.org/entity/Q1>\t\"1-n\"\t\"x\"@en\n";
        assert!(read(other.as_bytes()).is_err());
    }
}
