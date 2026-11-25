//! Stage 6: rendering (DESIGN.md section 4).
//!
//! A reply is a template with `{name}` placeholders; `{{` and `}}` stand for
//! literal braces. When a reply has several variants, which one is used is a
//! pure function of the conversation and the turn number
//! (docs/spec/conversation.md 5.2).

use std::collections::BTreeMap;

use crate::random::Fnv;
use crate::{ConvId, Error};

/// Variables for a template.
pub type Vars = BTreeMap<String, String>;

/// One piece of a parsed template.
#[derive(Debug, PartialEq, Eq)]
enum Piece<'a> {
    Text(&'a str),
    Var(&'a str),
}

/// Splits a template into literal text and variable names.
fn parse(template: &str) -> Result<Vec<Piece<'_>>, Error> {
    let mut pieces = Vec::new();
    let mut rest = template;
    while let Some(pos) = rest.find(['{', '}']) {
        if pos > 0 {
            pieces.push(Piece::Text(&rest[..pos]));
        }
        let tail = &rest[pos..];
        if let Some(after) = tail.strip_prefix("{{") {
            pieces.push(Piece::Text("{"));
            rest = after;
        } else if let Some(after) = tail.strip_prefix("}}") {
            pieces.push(Piece::Text("}"));
            rest = after;
        } else if tail.starts_with('}') {
            return Err(Error::Render(format!("unmatched `}}` in `{template}`")));
        } else {
            let end = tail
                .find('}')
                .ok_or_else(|| Error::Render(format!("unclosed `{{` in `{template}`")))?;
            let name = &tail[1..end];
            let valid =
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !valid {
                return Err(Error::Render(format!(
                    "bad variable name `{name}` in `{template}`"
                )));
            }
            pieces.push(Piece::Var(name));
            rest = &tail[end + 1..];
        }
    }
    if !rest.is_empty() {
        pieces.push(Piece::Text(rest));
    }
    Ok(pieces)
}

/// Checks that a template is well formed, for manifest loading.
pub(crate) fn check(template: &str) -> Result<(), Error> {
    parse(template).map(|_| ())
}

/// The variable names a well-formed template uses, sorted, without repeats;
/// empty for a malformed one.
#[must_use]
pub fn variables(template: &str) -> Vec<&str> {
    let mut names: Vec<&str> = parse(template)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|piece| match piece {
            Piece::Var(name) => Some(name),
            Piece::Text(_) => None,
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Fills a template. Every variable it names must be provided.
///
/// # Errors
///
/// [`Error::Render`] if the template is malformed or a variable is missing.
pub fn fill(template: &str, vars: &Vars) -> Result<String, Error> {
    let mut out = String::with_capacity(template.len());
    for piece in parse(template)? {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Var(name) => out.push_str(vars.get(name).ok_or_else(|| {
                Error::Render(format!("variable `{name}` not provided for `{template}`"))
            })?),
        }
    }
    Ok(out)
}

/// Which of `count` variants a turn uses.
///
/// The first turn of a conversation always gets the first variant, the
/// canonical wording, so a one-shot question reads the same every time.
/// Later turns step through the variants one at a time, so two successive
/// turns never share a wording, from a phase derived from the conversation
/// (chosen so that turn 2 differs from turn 1), so that conversations with
/// three or more variants do not vary in lock-step.
#[must_use]
pub fn variant(conversation: &ConvId, turn: u32, count: usize) -> usize {
    if count <= 1 || turn <= 1 {
        return 0;
    }
    let count = count as u64;
    let phase = Fnv::new().bytes(conversation.as_str().as_bytes()).finish() % (count - 1);
    let step = (u64::from(turn) - 1 + phase) % count;
    // The remainder is below `count`, which is a usize, so it fits.
    usize::try_from(step).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vars {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    #[test]
    fn fills_named_variables() {
        let out = fill(
            "En {place} son las {time}.",
            &vars(&[("place", "Tokio"), ("time", "03:42")]),
        );
        assert_eq!(out.unwrap(), "En Tokio son las 03:42.");
    }

    #[test]
    fn plain_text_and_escaped_braces() {
        assert_eq!(fill("¡Hola!", &Vars::new()).unwrap(), "¡Hola!");
        assert_eq!(
            fill("{{x}} = {x}", &vars(&[("x", "1")])).unwrap(),
            "{x} = 1"
        );
    }

    #[test]
    fn rejects_malformed_templates_and_missing_variables() {
        assert!(fill("{time", &Vars::new()).is_err());
        assert!(fill("time}", &Vars::new()).is_err());
        assert!(fill("{}", &Vars::new()).is_err());
        assert!(fill("{a b}", &Vars::new()).is_err());
        assert!(fill("{time}", &Vars::new()).is_err());
    }

    #[test]
    fn lists_variables() {
        assert_eq!(variables("{b} y {a}, {b}"), ["a", "b"]);
        assert_eq!(variables("{oops"), Vec::<&str>::new());
    }

    #[test]
    fn variant_is_a_pure_function_of_conversation_and_turn() {
        let a = ConvId::new("a");
        assert_eq!(variant(&a, 1, 3), 0);
        assert_eq!(variant(&a, 0, 3), 0);
        assert_eq!(variant(&a, 7, 1), 0);
        for turn in 2..20 {
            let v = variant(&a, turn, 3);
            assert!(v < 3);
            assert_eq!(v, variant(&a, turn, 3));
        }
        // Successive turns move through the variants, turn 2 included.
        for count in 2..6 {
            for turn in 1..20 {
                assert_ne!(variant(&a, turn, count), variant(&a, turn + 1, count));
            }
        }
    }
}
