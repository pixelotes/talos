//! `wikipedia`: what Wikipedia says about a subject, when the network is
//! allowed (docs/SKILLS.md; the manifest says what belongs here).
//!
//! One request per language tried: a search of the conversation's
//! Wikipedia that brings, with the five best matches, the opening of each
//! as plain text, its address and whether it is a page that only lists
//! subjects of the same name. The best match is answered with its opening
//! as written, cut at a sentence end, its title and its address; a list
//! page is asked about ("which one?") with the other matches as options. A
//! subject the conversation's Wikipedia lacks is looked up in the other
//! language's, and the reply says so (fallback `other-language`).
//!
//! Without the network, or when Wikipedia cannot be reached, the reply
//! says which (`Unable`): nothing is answered from memory.

use talos_core::net::{FetchError, encode};
use talos_core::{
    Call, Choice, Context, Expectation, Fallback, Lang, Message, Skill, SkillResult, Value,
};

use crate::dict::{join, piece, text, unquote};

/// The `wikipedia` handler.
#[derive(Debug, Default)]
pub(crate) struct Wikipedia;

const SKILL: &str = "wikipedia";

const REPLIES: &[&str] = &[
    "about",
    "ask_subject",
    "ask_article",
    "ambiguous",
    "not_found",
    "no_network",
    "failed",
    "p_article",
    "p_other_language",
    "word_or",
];

/// How many matches a search brings, and how many are offered as options.
const MATCHES: usize = 5;
const OPTIONS: usize = 4;
/// The longest opening quoted, in characters.
const OPENING: usize = 480;

/// Subjects that point at the speakers (Talos by name too: who it is, is
/// `identity`'s to say) or at something said before, not at anything an
/// encyclopedia has (match form).
const POINTERS: &[&str] = &[
    "ti", "tu", "usted", "vos", "vosotros", "mi", "yo", "nosotros", "eso", "esto", "ello", "el",
    "ella", "ellos", "ellas", "you", "yourself", "me", "myself", "us", "that", "this", "it",
    "him", "her", "them", "talos",
];

/// First words of what is not a subject: something of the speakers', or
/// a comparison asking for an opinion (match form).
const NOT_SUBJECTS: &[&str] = &[
    "tus", "mis", "su", "sus", "ese", "esa", "este", "esta", "esos", "esas", "estos", "estas",
    "your", "my", "his", "their", "our", "these", "those", "mejor", "peor", "mas", "menos",
    "best", "worst", "more", "less", "faster", "better", "bigger",
];

impl Skill for Wikipedia {
    fn id(&self) -> &'static str {
        SKILL
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "wikipedia.about" => about(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

/// One match of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Page {
    pub(crate) title: String,
    /// The opening, plain text, as Wikipedia gives it.
    pub(crate) extract: String,
    pub(crate) url: String,
    /// A page that only lists the subjects of that name.
    pub(crate) list: bool,
}

/// What looking a subject up gave.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Found {
    /// An article.
    Article(Page),
    /// A list page, and the other matches it may mean.
    Several(Vec<String>),
    /// Nothing by that name.
    Nothing,
    /// Wikipedia could not be reached or answered something unreadable.
    Failed,
    /// The network is not allowed.
    Off,
}

fn url(query: &str, lang: Lang) -> String {
    format!(
        "https://{}.wikipedia.org/w/api.php?action=query&format=json&formatversion=2\
         &redirects=1&generator=search&gsrnamespace=0&gsrlimit={MATCHES}&gsrsearch={}\
         &prop=extracts%7Cpageprops%7Cinfo&exintro=1&explaintext=1&exlimit={MATCHES}\
         &ppprop=disambiguation&inprop=url",
        lang.code(),
        encode(query)
    )
}

/// The matches of a search's answer, best first. `None` when it is not
/// the JSON expected.
pub(crate) fn pages(body: &str) -> Option<Vec<Page>> {
    let json: serde_json::Value = serde_json::from_str(body).ok()?;
    let Some(pages) = json.get("query").and_then(|q| q.get("pages")) else {
        // A search with no match has no `query` at all.
        return json.is_object().then(Vec::new);
    };
    let mut found: Vec<(u64, Page)> = Vec::new();
    for page in pages.as_array()? {
        let field = |name: &str| page.get(name).and_then(serde_json::Value::as_str);
        let (Some(title), Some(url)) = (field("title"), field("fullurl")) else {
            continue;
        };
        let index = page
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(u64::MAX);
        found.push((
            index,
            Page {
                title: title.to_owned(),
                extract: field("extract").unwrap_or_default().trim().to_owned(),
                url: url.to_owned(),
                list: page
                    .get("pageprops")
                    .is_some_and(|p| p.get("disambiguation").is_some()),
            },
        ));
    }
    found.sort_by_key(|(index, _)| *index);
    Some(found.into_iter().map(|(_, page)| page).collect())
}

/// A subject without the article it was said with ("la Segunda Guerra
/// Mundial"): with it the search prefers a title that has it, a book
/// called exactly that over the war. Kept when nothing else is left.
fn bare(subject: &str) -> &str {
    const ARTICLES: &[&str] = &["el", "la", "los", "las", "un", "una", "unos", "unas", "the", "a", "an"];
    match subject.split_once(' ') {
        Some((first, rest))
            if ARTICLES.contains(&first.to_lowercase().as_str()) && !rest.trim().is_empty() =>
        {
            rest.trim()
        }
        _ => subject,
    }
}

/// Looks `query` up in the Wikipedia of `lang`: the best match, or, when
/// `exact` names a title among the matches, that one.
pub(crate) fn find(cx: &Context<'_>, query: &str, exact: Option<&str>, lang: Lang) -> Found {
    let Some(network) = cx.network() else {
        return Found::Off;
    };
    let body = match network.get(&url(exact.unwrap_or_else(|| bare(query)), lang)) {
        Ok(body) => body,
        Err(FetchError::NotFound) => return Found::Nothing,
        Err(FetchError::Failed(_)) => return Found::Failed,
    };
    let Some(mut pages) = pages(&body) else {
        return Found::Failed;
    };
    if let Some(title) = exact
        && let Some(at) = pages.iter().position(|p| p.title == title)
    {
        pages.swap(0, at);
    }
    let Some(best) = pages.first() else {
        return Found::Nothing;
    };
    if best.list {
        let others: Vec<String> = pages
            .iter()
            .filter(|p| !p.list && !p.extract.is_empty())
            .take(OPTIONS)
            .map(|p| p.title.clone())
            .collect();
        return Found::Several(others);
    }
    match pages.into_iter().find(|p| !p.list && !p.extract.is_empty()) {
        Some(page) => Found::Article(page),
        None => Found::Nothing,
    }
}

/// The start of an opening: its first paragraph (with the second when the
/// first is only a line), cut at the last sentence end within [`OPENING`]
/// characters, or at a word with "…" when it has none.
pub(crate) fn opening(extract: &str) -> String {
    // Where a footnote mark was, the plain text keeps a zero-width space,
    // often with a space before the comma that followed it.
    let extract: String = extract
        .chars()
        .map(|c| match c {
            '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' => ' ',
            c => c,
        })
        .collect();
    let tidy = |p: &str| {
        p.split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace(" ,", ",")
            .replace(" .", ".")
            .replace("( ", "(")
            .replace(" )", ")")
            // What a pronunciation left behind when its audio was dropped.
            .replace(" ()", "")
    };
    let mut paragraphs = extract.split('\n').map(tidy).filter(|p| !p.is_empty());
    let mut text = paragraphs.next().unwrap_or_default();
    if text.chars().count() < 80
        && let Some(next) = paragraphs.next()
    {
        text.push(' ');
        text.push_str(&next);
    }
    if text.chars().count() <= OPENING {
        return text;
    }
    let end = text
        .char_indices()
        .nth(OPENING)
        .map_or(text.len(), |(i, _)| i);
    let head = &text[..end];
    if let Some(stop) = head.rfind(". ").filter(|stop| *stop > OPENING / 3) {
        return head[..=stop].to_owned();
    }
    let word = head.rfind(' ').unwrap_or(head.len());
    format!("{}…", head[..word].trim_end_matches([',', ';', ':']))
}

/// The article as a reply's text: the opening, the title and the address.
pub(crate) fn article_text(cx: &Context<'_>, lang: Lang, page: &Page) -> String {
    piece(
        cx,
        SKILL,
        lang,
        "p_article",
        &[
            ("extract", &opening(&page.extract)),
            ("title", &page.title),
            ("url", &page.url),
        ],
    )
}

/// The names an option answers to: its title, the title without what is
/// in brackets, and what is in brackets ("Mercurio (planeta)": "mercurio
/// (planeta)", "planeta").
fn names(title: &str) -> Vec<String> {
    let mut names = vec![title.to_owned()];
    if let Some((_, rest)) = title.split_once('(')
        && let Some((inside, _)) = rest.split_once(')')
        && !inside.trim().is_empty()
    {
        names.push(inside.trim().to_owned());
    }
    names
}

fn about(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let lang = call.lang;
    let Some(subject) = text(call.slots, "subject")
        .map(unquote)
        .filter(|s| !s.is_empty())
    else {
        if cx.guessed() {
            return SkillResult::Decline;
        }
        return SkillResult::Ask(
            Expectation::slot("subject"),
            Message::new("ask_subject"),
        );
    };
    // "Quién es tu cantante favorito", "who is the best footballer": about
    // the speakers, something pointed at, or an opinion; no article's.
    let key = talos_core::user::Correction::key(bare(&subject));
    let first = key.split(' ').next().unwrap_or_default();
    if POINTERS.contains(&first) || NOT_SUBJECTS.contains(&first) {
        return SkillResult::Decline;
    }
    // The title chosen comes back through the slot's reader, which trims
    // the punctuation at its end: "Mercurio (planeta" gets its bracket back.
    let article = text(call.slots, "article").map(|title| {
        if title.matches('(').count() > title.matches(')').count() {
            format!("{title})")
        } else {
            title.to_owned()
        }
    });
    cx.report("subject", Value::Text(subject.clone()));
    let mut found = find(cx, &subject, article.as_deref(), lang);
    let mut lines = Vec::new();
    if found == Found::Nothing && article.is_none() {
        let other = find(cx, &subject, None, lang.other());
        if matches!(other, Found::Article(_)) {
            cx.mark_fallback(Fallback::OTHER_LANGUAGE);
            lines.push(piece(cx, SKILL, lang, "p_other_language", &[]));
            found = other;
        }
    }
    match found {
        Found::Article(page) => {
            lines.push(article_text(cx, lang, &page));
            SkillResult::Answer(Message::new("about").var("text", lines.join("\n")))
        }
        Found::Several(options) if options.is_empty() => {
            SkillResult::Unable(Message::new("ambiguous").var("subject", subject))
        }
        Found::Several(options) => {
            let or = piece(cx, SKILL, lang, "word_or", &[]);
            let choices = options
                .iter()
                .map(|title| Choice::new(title.as_str(), names(title)))
                .collect();
            SkillResult::Ask(
                Expectation::choice("article", choices)
                    .keeping("subject", Value::Text(subject.clone())),
                Message::new("ask_article")
                    .var("subject", subject)
                    .var("options", join(&options, &or)),
            )
        }
        Found::Nothing => SkillResult::Unable(Message::new("not_found").var("subject", subject)),
        Found::Failed => SkillResult::Unable(Message::new("failed")),
        Found::Off => SkillResult::Unable(Message::new("no_network")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_answer_gives_its_pages_best_first() {
        let body = r#"{"batchcomplete":true,"query":{"pages":[
            {"pageid":2,"title":"Mercurio (planeta)","index":2,"extract":"Mercurio es el planeta…",
             "fullurl":"https://es.wikipedia.org/wiki/Mercurio_(planeta)"},
            {"pageid":1,"title":"Mercurio","index":1,"extract":"Mercurio puede referirse a:",
             "pageprops":{"disambiguation":""},"fullurl":"https://es.wikipedia.org/wiki/Mercurio"}]}}"#;
        let pages = pages(body).unwrap();
        assert_eq!(pages[0].title, "Mercurio");
        assert!(pages[0].list);
        assert!(!pages[1].list);
        assert_eq!(pages[1].url, "https://es.wikipedia.org/wiki/Mercurio_(planeta)");
        assert_eq!(super::pages(r#"{"batchcomplete":true}"#), Some(Vec::new()));
        assert_eq!(super::pages("<html>busy</html>"), None);
    }

    #[test]
    fn an_opening_is_cut_at_a_sentence() {
        assert_eq!(
            opening("Corta.\nOtro párrafo que también cuenta."),
            "Corta. Otro párrafo que también cuenta."
        );
        assert_eq!(
            opening("Valencia (València,\u{200b} AFI)\u{200b} es una ciudad de España,\u{200b} con 840 792  habitantes ."),
            "Valencia (València, AFI) es una ciudad de España, con 840 792 habitantes."
        );
        assert_eq!(
            opening("Lisboa (pronunciación: /liʒˈβoɐ/ ()) es la capital de Portugal.\u{200b}Está en el Tajo."),
            "Lisboa (pronunciación: /liʒˈβoɐ/) es la capital de Portugal. Está en el Tajo."
        );
        assert_eq!(bare("la Segunda Guerra Mundial"), "Segunda Guerra Mundial");
        assert_eq!(bare("The Beatles"), "Beatles");
        assert_eq!(bare("la"), "la");
        let long = format!("{} Fin de la segunda. {}", "Primera frase larga. ".repeat(20), "x ".repeat(200));
        let cut = opening(&long);
        assert!(cut.chars().count() <= OPENING + 1, "{}", cut.chars().count());
        assert!(cut.ends_with('.'), "{cut}");
        let unbroken = "palabra ".repeat(200);
        assert!(opening(&unbroken).ends_with("palabra…"));
    }

    #[test]
    fn an_option_answers_to_what_is_in_brackets() {
        assert_eq!(names("Mercurio (planeta)"), ["Mercurio (planeta)", "planeta"]);
        assert_eq!(names("Valencia"), ["Valencia"]);
    }

    #[test]
    fn the_search_address_encodes_the_subject() {
        let url = url("Ramón y Cajal", Lang::Es);
        assert!(url.starts_with("https://es.wikipedia.org/w/api.php?"), "{url}");
        assert!(url.contains("gsrsearch=Ram%C3%B3n%20y%20Cajal&"), "{url}");
    }
}
