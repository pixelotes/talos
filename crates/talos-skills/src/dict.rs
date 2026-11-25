//! What `define`, `translate` and `teach` share: wording read from their
//! manifests (so that no sentence is written in code), lists, quotes and
//! the cleaning of a captured word.

use talos_core::normalise::normalise;
use talos_core::render::{self, Vars};
use talos_core::{Context, Lang, Value};

/// A reply of `skill`'s manifest, filled in, for a part of a sentence the
/// handler composes (the `*` pieces of the manifest, never answered on
/// their own). The id itself outside an engine (unit tests), so a missing
/// piece shows rather than vanishes.
pub(crate) fn piece(
    cx: &Context<'_>,
    skill: &str,
    lang: Lang,
    id: &str,
    vars: &[(&str, &str)],
) -> String {
    let Some(template) = cx
        .skills()
        .iter()
        .find(|m| m.id == skill)
        .and_then(|m| m.replies.get(id))
        .and_then(|r| r.get(lang).first())
    else {
        return id.to_owned();
    };
    let vars: Vars = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    render::fill(template, &vars).unwrap_or_else(|_| template.clone())
}

/// "a", "a and b", "a, b and c", with the conjunction given.
pub(crate) fn join(items: &[String], conjunction: &str) -> String {
    match items {
        [] => String::new(),
        [only] => only.clone(),
        [init @ .., last] => format!("{} {conjunction} {last}", init.join(", ")),
    }
}

/// The first letter in upper case.
pub(crate) fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Quotes and edge punctuation a captured word may carry.
const EDGES: &[char] = &[
    '"', '\'', '«', '»', '“', '”', '‘', '’', '¿', '?', '¡', '!', '.', ',', ';', ':', '(', ')',
];

/// A captured word or phrase without the quotes and punctuation around it.
pub(crate) fn unquote(text: &str) -> String {
    text.trim_matches(|c: char| c.is_whitespace() || EDGES.contains(&c))
        .to_owned()
}

/// A captured phrase without the quotes around it, its own punctuation
/// kept ("«¿y hoy?»" is "¿y hoy?").
pub(crate) fn strip_quotes(text: &str) -> String {
    const QUOTES: &[char] = &['"', '\'', '«', '»', '“', '”', '‘', '’'];
    text.trim()
        .trim_matches(|c: char| c.is_whitespace() || QUOTES.contains(&c) || c == ',')
        .to_owned()
}

/// Articles a definition or translation request may put before its word
/// ("qué es *una* guitarra", "what's *a* fret").
const ARTICLES: &[&str] = &[
    "un", "una", "unos", "unas", "el", "la", "los", "las", "lo", "a", "an", "the",
];

/// Words that point at something instead of naming it: "¿qué es esto?",
/// "what's that?". With no topic, Talos asks which word.
const DEICTIC: &[&str] = &[
    "esto", "eso", "aquello", "este", "esta", "ese", "esa", "it", "that", "this", "those", "these",
    "lo",
];

/// A captured word ready to look up: unquoted, without a leading article;
/// `None` when nothing is left or it only points ("esto", "that").
pub(crate) fn clean_word(text: &str) -> Option<String> {
    let text = unquote(text);
    let mut words: Vec<&str> = text.split_whitespace().collect();
    while words.len() > 1 && ARTICLES.contains(&normalise(words[0]).folded.as_str()) {
        words.remove(0);
    }
    let word = unquote(&words.join(" "));
    let folded = normalise(&word).folded;
    let first = folded.split_whitespace().next().unwrap_or("");
    if word.is_empty() || (folded.split_whitespace().count() <= 2 && DEICTIC.contains(&first)) {
        return None;
    }
    Some(word)
}

/// A slot's text, when it has some.
pub(crate) fn text<'a>(slots: &'a talos_core::Slots, name: &str) -> Option<&'a str> {
    slots
        .get(name)
        .and_then(Value::as_text)
        .filter(|t| !t.trim().is_empty())
}

/// The language a `language` slot names, when it is one Talos speaks;
/// `Err(())` when it names another one. Words that name no language at all
/// ("en realidad") are no target: `Ok(None)`.
#[cfg(feature = "knowledge")]
pub(crate) fn language(slots: &talos_core::Slots, name: &str) -> Result<Option<Lang>, ()> {
    match slots.get(name) {
        Some(Value::Language(code)) => code.parse::<Lang>().map(Some).map_err(|_| ()),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_are_cleaned_of_quotes_articles_and_pointers() {
        assert_eq!(clean_word("«una guitarra»").as_deref(), Some("guitarra"));
        assert_eq!(clean_word("the sound box").as_deref(), Some("sound box"));
        assert_eq!(clean_word(" \"fret\"? ").as_deref(), Some("fret"));
        assert_eq!(clean_word("esto"), None);
        assert_eq!(clean_word("that"), None);
        assert_eq!(clean_word("la"), Some("la".to_owned()));
        assert_eq!(clean_word("los violines").as_deref(), Some("violines"));
    }

    #[test]
    fn lists_join_with_the_conjunction() {
        let items = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(join(&items(&["a"]), "y"), "a");
        assert_eq!(join(&items(&["a", "b", "c"]), "o"), "a, b o c");
        assert_eq!(capitalise("ñu"), "Ñu");
    }
}
