//! Stage 7: the reflector, the fallback when nothing answered
//! (docs/spec/reflector.md, ADR 0009).
//!
//! It recognises the *shape* of a sentence and hands the conversation back,
//! sometimes echoing the user's words with the persons swapped. It never
//! answers: a recognised shape is `Deflected`; a guard (a factual question,
//! a request for text, noise, another language, long text), a request
//! Talos cannot meet, and the bare catch-all are `Unhandled`. The shape or
//! the reason travels in [`crate::Outcome::reason`]; the intent stays empty.
//!
//! One script per language, written natively, embedded from `reflector/`
//! at the workspace root ([`script`]); the person tables are in the same
//! files ([`reflect`]). Everything is compiled once, when the engine is
//! built.

pub(crate) mod guards;
pub(crate) mod reflect;
pub(crate) mod script;

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::ops::Range;

use self::reflect::{MAX_ECHO_WORDS, Reflection, Tables};
use self::script::{Person, Rule, Script, Template, When};
use crate::normalise::Normalised;
use crate::patterns::Patterns;
use crate::state::ConversationState;
use crate::{ConvId, Error, Lang, Reason, Status, render};

/// The Spanish script.
pub(crate) const SCRIPT_ES: &str = include_str!("../../../../reflector/es.toml");
/// The English script.
pub(crate) const SCRIPT_EN: &str = include_str!("../../../../reflector/en.toml");

/// Memory keys for replies that belong to no rule.
const KEY_CATCH_ALL: u16 = u16::MAX;
const KEY_NOISE: u16 = u16::MAX - 1;
const KEY_OTHER_LANGUAGE: u16 = u16::MAX - 2;
const KEY_LONG_TEXT: u16 = u16::MAX - 3;

/// Both scripts, compiled.
#[derive(Debug)]
pub(crate) struct Reflector {
    scripts: [Script; 2],
    tables: [Tables; 2],
}

/// What the reflector (or the safety check) says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reply {
    pub(crate) text: String,
    pub(crate) status: Status,
    pub(crate) reason: Reason,
    /// For the trace.
    pub(crate) note: String,
}

/// One sentence of the match form.
#[derive(Debug, Clone)]
struct Sentence {
    /// Byte range in the match form, punctuation at the ends trimmed.
    range: Range<usize>,
    question: bool,
}

/// A rule that matched a sentence.
#[derive(Debug)]
struct Found<'a> {
    rule: &'a Rule,
    index: usize,
    pattern: &'a str,
    captures: BTreeMap<String, Reflection>,
}

impl Reflector {
    /// Loads the embedded scripts.
    ///
    /// # Errors
    ///
    /// [`Error::Script`] if a script is malformed.
    pub(crate) fn new() -> Result<Self, Error> {
        Self::from_sources(SCRIPT_ES, SCRIPT_EN)
    }

    /// Loads two scripts, Spanish first.
    pub(crate) fn from_sources(es: &str, en: &str) -> Result<Self, Error> {
        let mut es = Script::parse("reflector/es.toml", es)?;
        let mut en = Script::parse("reflector/en.toml", en)?;
        for (script, lang) in [(&es, Lang::Es), (&en, Lang::En)] {
            if script.lang != lang {
                return Err(Error::Script {
                    script: format!("reflector/{lang}.toml"),
                    message: format!("declares `lang = \"{}\"`", script.lang),
                });
            }
        }
        // The tables as written are not needed once compiled.
        let tables = |script: &mut Script| {
            let raw = std::mem::take(&mut script.reflect);
            Tables::compile(script.lang, &raw).map_err(|message| Error::Script {
                script: format!("reflector/{}.toml", script.lang),
                message,
            })
        };
        let tables = [tables(&mut es)?, tables(&mut en)?];
        Ok(Self {
            scripts: [es, en],
            tables,
        })
    }

    pub(crate) fn script(&self, lang: Lang) -> &Script {
        &self.scripts[lang.index()]
    }

    fn tables(&self, lang: Lang) -> &Tables {
        &self.tables[lang.index()]
    }

    /// Answers a message nothing else took.
    pub(crate) fn respond(
        &self,
        lang: Lang,
        message: &Normalised,
        conversation: &ConvId,
        turn: u32,
        state: &mut ConversationState,
    ) -> Reply {
        let script = self.script(lang);
        let cx = (lang, conversation, turn);
        if guards::is_long_text(message) {
            return guard(
                cx,
                state,
                &script.long_text,
                KEY_LONG_TEXT,
                "long-text",
                &format!("over {} words, or several lines", guards::LONG_TEXT_WORDS),
            );
        }
        if guards::is_other_language(message) {
            return guard(
                cx,
                state,
                &script.other_language,
                KEY_OTHER_LANGUAGE,
                "other-language",
                "more words of other languages than of Spanish and English",
            );
        }

        let sentences = sentences(&message.folded);
        if let Some(found) = self.find(lang, message, &sentences) {
            return self.reply(lang, lang, &found, conversation, turn, state);
        }
        if let Some(found) = self.find(lang.other(), message, &sentences) {
            return self.reply(lang, lang.other(), &found, conversation, turn, state);
        }

        if guards::is_noise(message) {
            return guard(
                cx,
                state,
                &script.noise,
                KEY_NOISE,
                "noise",
                "no word in it looks like a word",
            );
        }
        let rules = script.rules.len();
        let (text, note) = choose(
            &script.catch_all,
            &BTreeMap::new(),
            conversation,
            turn,
            state,
            (lang, KEY_CATCH_ALL),
            false,
        );
        Reply {
            text,
            status: Status::Unhandled,
            reason: Reason::CATCH_ALL,
            note: format!(
                "catch-all: no shape matched ({rules} {lang} and {} {} rules tried); {note}",
                self.script(lang.other()).rules.len(),
                lang.other()
            ),
        }
    }

    /// The best rule over every sentence: the highest rank; on a tie, the
    /// last sentence; within a sentence, the full text before the one with
    /// fillers dropped.
    fn find<'a>(
        &'a self,
        lang: Lang,
        message: &Normalised,
        sentences: &[Sentence],
    ) -> Option<Found<'a>> {
        let script = self.script(lang);
        let tables = self.tables(lang);
        let mut best: Option<Found<'a>> = None;
        for sentence in sentences {
            let mut ranges = vec![sentence.range.clone()];
            let stripped = strip_fillers(&message.folded, sentence.range.clone(), &script.fillers);
            if stripped != sentence.range && !stripped.is_empty() {
                ranges.push(stripped);
            }
            let mut here: Option<Found<'a>> = None;
            for range in ranges {
                let text = &message.folded[range.clone()];
                let matched = script.patterns.matches(text);
                let mut rules: Vec<usize> =
                    matched.iter().map(|&p| script.pattern_rule[p]).collect();
                rules.dedup();
                for index in rules {
                    if here.as_ref().is_some_and(|h| h.index <= index) {
                        break;
                    }
                    let rule = &script.rules[index];
                    if let Some(found) = try_rule(
                        (rule, index),
                        (&script.patterns, &matched),
                        tables,
                        message,
                        &range,
                        sentence.question,
                    ) {
                        here = Some(found);
                        break;
                    }
                }
            }
            if let Some(here) = here
                && best.as_ref().is_none_or(|b| here.rule.rank <= b.rule.rank)
            {
                best = Some(here);
            }
        }
        best
    }

    /// Renders the reply for a rule found in `matched`'s script, in `lang`.
    fn reply(
        &self,
        lang: Lang,
        matched: Lang,
        found: &Found<'_>,
        conversation: &ConvId,
        turn: u32,
        state: &mut ConversationState,
    ) -> Reply {
        let shape = found.rule.shape;
        let mut note = format!(
            "shape {shape} ({} rule {}, /{}/)",
            matched,
            found.index + 1,
            found.pattern
        );
        let (rule, index, captures) = if matched == lang {
            (found.rule, found.index, found.captures.clone())
        } else {
            // Recognised by the other language's script: reply in this
            // language with the shape's first rule, echoing nothing.
            let script = self.script(lang);
            match script.rules.iter().position(|r| r.shape == shape) {
                Some(index) => {
                    let _ = write!(note, "; replied with {lang} rule {}, no echo", index + 1);
                    (&script.rules[index], index, BTreeMap::new())
                }
                None => (found.rule, found.index, BTreeMap::new()),
            }
        };
        for (name, reflection) in &captures {
            match &reflection.echo {
                Ok(echo) => {
                    let _ = write!(note, "; {name}={echo:?}");
                }
                Err(why) => {
                    let _ = write!(note, "; {name} not echoed: {why}");
                }
            }
        }
        let key = u16::try_from(index).unwrap_or(KEY_LONG_TEXT - 1);
        let thin = rule.status == Status::Deflected;
        let (text, chosen) = choose(
            &rule.replies,
            &captures,
            conversation,
            turn,
            state,
            (lang, key),
            thin,
        );
        let _ = write!(note, "; {chosen}");
        Reply {
            text,
            status: rule.status,
            reason: Reason::new(shape),
            note,
        }
    }
}

/// The reply of a guard decided in code.
fn guard(
    (lang, conversation, turn): (Lang, &ConvId, u32),
    state: &mut ConversationState,
    templates: &[Template],
    key: u16,
    reason: &'static str,
    why: &str,
) -> Reply {
    let (text, note) = choose(
        templates,
        &BTreeMap::new(),
        conversation,
        turn,
        state,
        (lang, key),
        false,
    );
    Reply {
        text,
        status: Status::Unhandled,
        reason: Reason::new(reason),
        note: format!("guard {reason}: {why}; {note}"),
    }
}

/// Tries one rule on one sentence. `matched` lists the patterns that match
/// it, so the others are not run (nor built) again to read captures.
fn try_rule<'a>(
    (rule, index): (&'a Rule, usize),
    (patterns, matched): (&'a Patterns, &[usize]),
    tables: &Tables,
    message: &Normalised,
    range: &Range<usize>,
    question: bool,
) -> Option<Found<'a>> {
    let wrong_kind = match rule.when {
        When::Any => false,
        When::Question => !question,
        When::Statement => question,
    };
    if wrong_kind {
        return None;
    }
    let text = &message.folded[range.clone()];
    if (0..rule.unless.len()).any(|u| rule.unless.is_match(u, text)) {
        return None;
    }
    if rule.person == Person::None && tables.has_person(message.original_of(range.clone())) {
        return None;
    }
    for pattern in rule
        .patterns
        .clone()
        .filter(|p| matched.binary_search(p).is_ok())
    {
        let Some(caps) = patterns.captures(pattern, text) else {
            continue;
        };
        let mut captures = BTreeMap::new();
        for (name, m) in caps {
            let absolute = range.start + m.start..range.start + m.end;
            let before = text[..m.start]
                .split(|c: char| !c.is_alphanumeric())
                .rfind(|w| !w.is_empty());
            let original = message.original_of(absolute);
            captures.insert(name.to_owned(), tables.reflect(original, before));
        }
        if rule.person == Person::First && !captures.values().any(|r| r.first) {
            return None;
        }
        return Some(Found {
            rule,
            index,
            pattern: patterns.source(pattern),
            captures,
        });
    }
    None
}

/// Splits the match form into sentences at `.`, `!`, `?`, `;` and before
/// `¿` or `¡`, with punctuation trimmed from both ends.
fn sentences(folded: &str) -> Vec<Sentence> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut push = |range: Range<usize>| {
        let piece = &folded[range.clone()];
        let question = piece.contains(['?', '¿']);
        let trimmed_start = piece.len() - piece.trim_start_matches(is_edge).len();
        let trimmed_end = piece.trim_end_matches(is_edge).len();
        if trimmed_end > trimmed_start {
            out.push(Sentence {
                range: range.start + trimmed_start..range.start + trimmed_end,
                question,
            });
        }
    };
    for (i, c) in folded.char_indices() {
        if matches!(c, '¿' | '¡') && i > start {
            push(start..i);
            start = i;
        } else if matches!(c, '.' | '!' | '?' | ';' | '…') {
            let end = i + c.len_utf8();
            push(start..end);
            start = end;
        }
    }
    if start < folded.len() {
        push(start..folded.len());
    }
    out
}

fn is_edge(c: char) -> bool {
    c.is_whitespace() || ".,;:!?¿¡…\"'«»()[]-".contains(c)
}

/// Drops fillers ("pues", "bueno", "well") from the start of a sentence,
/// and Talos's name used as a vocative from its end ("cállate, Talos").
fn strip_fillers(folded: &str, mut range: Range<usize>, fillers: &[String]) -> Range<usize> {
    let separators = [' ', ','];
    loop {
        let text = &folded[range.clone()];
        let Some(filler) = fillers.iter().find(|f| {
            text.strip_prefix(f.as_str()).is_some_and(|rest| {
                rest.starts_with(separators) && !rest.trim_start_matches(separators).is_empty()
            })
        }) else {
            break;
        };
        let rest = &text[filler.len()..];
        range.start += filler.len() + (rest.len() - rest.trim_start_matches(separators).len());
    }
    let text = &folded[range.clone()];
    if let Some(rest) = text.strip_suffix("talos") {
        let kept = rest.trim_end_matches(separators);
        if kept.len() < rest.len() && !kept.is_empty() {
            range.end = range.start + kept.len();
        }
    }
    range
}

/// Picks a variant (docs/spec/reflector.md 6): a pure function of the
/// conversation and the turn number, skipping the variant this rule used
/// last time and the previous reply's text; after two deflections in a row
/// that ended in a question, one without. Replies whose echo is blocked
/// are left out. Returns the text and a note for the trace.
fn choose(
    templates: &[Template],
    captures: &BTreeMap<String, Reflection>,
    conversation: &ConvId,
    turn: u32,
    state: &mut ConversationState,
    (lang, key): (Lang, u16),
    thin_questions: bool,
) -> (String, String) {
    let echo = |name: &str| -> Option<&str> {
        match captures.get(name) {
            None => Some(""),
            Some(r) => r.echo.as_deref().ok(),
        }
    };
    let mut eligible: Vec<(usize, String)> = templates
        .iter()
        .enumerate()
        .filter_map(|(i, t)| {
            let mut vars = render::Vars::new();
            let mut words = 0;
            for name in &t.vars {
                let text = echo(name)?;
                words += text.split_whitespace().count();
                vars.insert(name.clone(), text.to_owned());
            }
            if words > MAX_ECHO_WORDS {
                return None;
            }
            render::fill(&t.text, &vars)
                .ok()
                .map(|text| (i, tidy(&text)))
        })
        .collect();
    let echoes = eligible.iter().any(|(i, _)| !templates[*i].is_neutral());
    let memory = state.recent();
    if thin_questions && memory.questions_in_row >= 2 {
        let plain: Vec<(usize, String)> = eligible
            .iter()
            .filter(|(_, text)| !text.ends_with('?'))
            .cloned()
            .collect();
        if !plain.is_empty() {
            eligible = plain;
        }
    }
    let Some(first) = eligible.first().cloned() else {
        // Validation guarantees a neutral reply; this is unreachable in
        // practice, and says something honest if it is not.
        return (String::new(), "no eligible variant".to_owned());
    };
    let count = eligible.len();
    let start = render::variant(conversation, turn, count);
    let last_variant = memory.variant(lang, key);
    let last_reply = memory.last_reply.as_deref();
    let chosen = (0..count)
        .map(|k| &eligible[(start + k) % count])
        .find(|(i, text)| {
            count == 1 || (Some(*i) != last_variant && Some(text.as_str()) != last_reply)
        })
        .or_else(|| {
            (0..count)
                .map(|k| &eligible[(start + k) % count])
                .find(|(_, text)| Some(text.as_str()) != last_reply)
        })
        .unwrap_or(&first)
        .clone();
    state.recent_mut().set_variant(lang, key, chosen.0);
    let note = format!(
        "variant {} of {} ({} eligible{})",
        chosen.0 + 1,
        templates.len(),
        count,
        if echoes {
            ", echo allowed"
        } else {
            ", no echo"
        }
    );
    (chosen.1, note)
}

/// Collapses spaces, closes up space before punctuation, and capitalises
/// the first letter (after an opening `¿` or `¡`).
fn tidy(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        let joins = word.starts_with([',', '.', '?', '!', ';', ':']);
        if !out.is_empty() && !joins {
            out.push(' ');
        }
        out.push_str(word);
    }
    let out = out.replace("¿ ", "¿").replace("¡ ", "¡").replace(" ,", ",");
    let mut result = String::with_capacity(out.len());
    let mut done = false;
    for c in out.chars() {
        if !done && c.is_alphabetic() {
            result.extend(c.to_uppercase());
            done = true;
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests;
