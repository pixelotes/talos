//! `translate`: words and stock phrases between Spanish and English
//! (docs/spec/knowledge.md, `translate.word`).
//!
//! In order: a stock phrase of the table (its counterpart, with the note
//! that tells variants apart, or the nearest match when there is no fixed
//! one); a word of the concept store, translated through its concept (one
//! translation per sense unless the sense is known from memory or context,
//! keeping the plural of what was typed); failing that, the word-pair
//! lexicon, always hedged and marked `word-pairs`. A phrase that is none of
//! these is not translated: Talos says it translates words and set phrases
//! only, and when every word of it is known it adds a word-by-word
//! rendering, labelled approximate and marked `word-by-word`, the turn
//! staying `Unhandled` (DESIGN.md principle 8). A third language is
//! refused.

use talos_core::{Call, Context, Message, Skill, SkillResult};

/// The `translate` handler.
#[derive(Debug, Default)]
pub(crate) struct Translate;

/// Every reply id the handler can return or compose from.
const REPLIES: &[&str] = &[
    "translated",
    "in_lang",
    "same",
    "already",
    "senses",
    "phrase",
    "phrase_approx",
    "lexicon",
    "lexicon_also",
    "no_word",
    "unknown",
    "sentence",
    "word_by_word",
    "third_language",
    "ask_term",
    "no_dictionary",
    // Pieces.
    "p_word",
    "p_also",
    "p_sense",
    "p_noted",
    "name_es",
    "name_en",
    "word_and",
];

impl Skill for Translate {
    fn id(&self) -> &'static str {
        "translate"
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "translate.word" => word(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

#[cfg(not(feature = "knowledge"))]
fn word(_cx: &mut Context<'_>, _call: &Call<'_>) -> SkillResult {
    SkillResult::Unable(Message::new("no_dictionary"))
}

#[cfg(feature = "knowledge")]
fn word(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let Some(k) = cx.knowledge() else {
        return SkillResult::Unable(Message::new("no_dictionary"));
    };
    imp::translate(cx, k, call)
        .unwrap_or_else(|e| SkillResult::Fail(talos_core::SkillError(e.to_string())))
}

#[cfg(feature = "knowledge")]
mod imp {
    use talos_core::knowledge::{Knowledge, Sense, Step, StockPhrase, key};
    use talos_core::{
        Call, Context, Error, Expectation, Fallback, Lang, Message, SkillResult, Value,
    };

    use crate::define::imp::{choose, label};
    use crate::dict::{capitalise, clean_word, join, language, piece, text};

    const SKILL: &str = "translate";

    fn p(cx: &Context<'_>, lang: Lang, id: &str, vars: &[(&str, &str)]) -> String {
        piece(cx, SKILL, lang, id, vars)
    }

    fn name(cx: &Context<'_>, lang: Lang, of: Lang) -> String {
        p(
            cx,
            lang,
            match of {
                Lang::Es => "name_es",
                Lang::En => "name_en",
            },
            &[],
        )
    }

    /// A term whose last two words name a language ("banco al inglés"):
    /// the rest and that language; `Err` for a language Talos does not
    /// speak.
    fn trailing_language(term: &str) -> Result<(String, Option<Lang>), ()> {
        let words: Vec<&str> = term.split_whitespace().collect();
        if words.len() < 3 {
            return Ok((term.to_owned(), None));
        }
        let (rest, tail) = words.split_at(words.len() - 2);
        let folded = talos_core::normalise::normalise(&tail.join(" ")).folded;
        match talos_core::extract::language(&folded) {
            Ok(Value::Language(code)) => match code.parse::<Lang>() {
                Ok(lang) => Ok((rest.join(" "), Some(lang))),
                Err(_) => Err(()),
            },
            _ => Ok((term.to_owned(), None)),
        }
    }

    /// A word as shown in a reply.
    fn quoted(cx: &Context<'_>, lang: Lang, word: &str) -> String {
        p(cx, lang, "p_word", &[("word", word)])
    }

    /// The first word, then the others as "also"; the first capitalised
    /// when it opens the reply.
    fn words_text(cx: &Context<'_>, lang: Lang, words: &[String], opens: bool) -> String {
        let shown: Vec<String> = words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == 0 && opens {
                    quoted(cx, lang, &capitalise(w))
                } else {
                    quoted(cx, lang, w)
                }
            })
            .collect();
        match shown.split_first() {
            None => String::new(),
            Some((first, [])) => first.clone(),
            Some((first, rest)) => {
                let and = p(cx, lang, "word_and", &[]);
                p(
                    cx,
                    lang,
                    "p_also",
                    &[("first", first), ("rest", &join(rest, &and))],
                )
            }
        }
    }

    /// A concept's words in `target`, in the number of what was typed.
    fn targets(k: &Knowledge, sense: &Sense, target: Lang) -> Result<Vec<String>, Error> {
        let plural = sense.features.as_deref() == Some("pl");
        let mut out = Vec::new();
        for word in k.words(&sense.concept, target)? {
            let shown = if plural {
                k.form_of(&word, "pl")?.unwrap_or_else(|| word.lemma.clone())
            } else {
                word.lemma.clone()
            };
            if !out.contains(&shown) {
                out.push(shown);
            }
        }
        Ok(out)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the order of docs/spec/knowledge.md, translate: phrase, word, lexicon, refusal"
    )]
    pub(super) fn translate(
        cx: &mut Context<'_>,
        k: &Knowledge,
        call: &Call<'_>,
    ) -> Result<SkillResult, Error> {
        let lang = call.lang;
        let Ok(target) = language(call.slots, "target") else {
            return Ok(SkillResult::Unable(Message::new("third_language")));
        };
        let term = text(call.slots, "term")
            .and_then(clean_word)
            .or_else(|| cx.conversation().topic().map(str::to_owned));
        let Some(term) = term else {
            return Ok(SkillResult::Ask(
                Expectation::slot("term"),
                Message::new("ask_term"),
            ));
        };
        // "/traduce banco al inglés": the line is all term; its last words
        // may name the target.
        let (term, target) = match target {
            Some(t) => (term, Some(t)),
            None => match trailing_language(&term) {
                Ok(split) => split,
                Err(()) => return Ok(SkillResult::Unable(Message::new("third_language"))),
            },
        };
        let context = text(call.slots, "context").unwrap_or_default().to_owned();
        cx.set_topic(term.clone());
        cx.report("term", Value::Text(term.clone()));

        let phrases = k.stock_phrases(&term)?;
        if !phrases.is_empty() {
            return Ok(phrase(cx, &phrases, lang, target));
        }

        // Which language the term is in, and into which it goes.
        let (source, into, senses, already) = match target {
            Some(t) => {
                let from = k.senses_in(&term, t.other())?;
                if from.is_empty() {
                    let within = k.senses_in(&term, t)?;
                    let already = !within.is_empty();
                    (t.other(), t, within, already)
                } else {
                    (t.other(), t, from, false)
                }
            }
            None => match k.lookup(&term, lang)? {
                Some(found) => (found.lang, found.lang.other(), found.senses, false),
                None => (lang, lang.other(), Vec::new(), false),
            },
        };
        let (source, into) = if already { (into, source) } else { (source, into) };
        cx.report("target", Value::Language(into.code()));
        if senses.is_empty() {
            return unknown(cx, k, &term, source, target.is_none(), lang);
        }

        let choice = choose(cx, k, &senses, lang, &context)?;
        let decided = match choice.chosen {
            Some((i, Step::Remembered | Step::Turn | Step::Recent)) => Some(i),
            _ if senses.len() == 1 => Some(0),
            _ => None,
        };
        if let Some((i, Step::Turn)) = choice.chosen {
            cx.remember_sense(key(&senses[0].word.lemma), senses[i].concept.as_str());
        }
        let listed: Vec<usize> = decided.map_or_else(|| (0..senses.len()).collect(), |i| vec![i]);
        let mut found: Vec<(usize, Vec<String>)> = Vec::new();
        for i in listed {
            let words = targets(k, &senses[i], into)?;
            if !words.is_empty() {
                found.push((i, words));
            }
        }
        if let Some(i) = decided {
            cx.report("sense", Value::Text(senses[i].concept.as_str().to_owned()));
        }
        match found.as_slice() {
            [] => lexicon(cx, k, &term, source, lang, true),
            [(i, words)] => {
                cx.mention(senses[*i].concept.as_str());
                let how = if already {
                    How::Already
                } else if words.len() == 1 && key(&words[0]) == key(&senses[*i].word.lemma) {
                    How::Same
                } else if target.is_none() {
                    How::InLang
                } else {
                    How::Plain
                };
                Ok(SkillResult::Answer(one(cx, lang, &term, words, (source, into), how)))
            }
            several => {
                let mut items = Vec::new();
                for (i, words) in several {
                    let option = &choice.options[*i];
                    let shown_label = label(k, &option.concept, &option.label, lang, false)?;
                    let words = words_text(cx, lang, words, false);
                    items.push(p(
                        cx,
                        lang,
                        "p_sense",
                        &[("words", &words), ("label", &shown_label)],
                    ));
                }
                Ok(SkillResult::Answer(
                    Message::new("senses").var("items", items.join("; ")),
                ))
            }
        }
    }

    /// How a single translation is introduced.
    #[derive(Clone, Copy)]
    enum How {
        /// «Guitar».
        Plain,
        /// No target was asked: «En inglés, «guitar».»
        InLang,
        /// The same word in both languages: «Igual: «virus».»
        Same,
        /// The term was already in the target language asked for.
        Already,
    }

    /// The reply for one sense's translations.
    fn one(
        cx: &Context<'_>,
        lang: Lang,
        term: &str,
        words: &[String],
        (source, into): (Lang, Lang),
        how: How,
    ) -> Message {
        let shown = words_text(cx, lang, words, false);
        match how {
            How::Already => Message::new("already")
                .var("term", term.to_owned())
                .var("lang_name", name(cx, lang, source))
                .var("target_name", name(cx, lang, into))
                .var("words", shown),
            How::Same => Message::new("same").var("words", shown),
            How::InLang => Message::new("in_lang")
                .var("target_name", name(cx, lang, into))
                .var("words", shown),
            How::Plain => {
                Message::new("translated").var("words", words_text(cx, lang, words, true))
            }
        }
    }

    /// No word in the concept store: a phrase (word by word, or refused),
    /// or the lexicon, or an unknown word.
    fn unknown(
        cx: &mut Context<'_>,
        k: &Knowledge,
        term: &str,
        source: Lang,
        any_direction: bool,
        lang: Lang,
    ) -> Result<SkillResult, Error> {
        if term.split_whitespace().count() > 1 {
            return Ok(match word_by_word(cx, k, term, source)? {
                Some(rendered) => {
                    cx.mark_fallback(Fallback::WORD_BY_WORD);
                    SkillResult::Unable(Message::new("word_by_word").var("text", rendered))
                }
                None => SkillResult::Unable(Message::new("sentence")),
            });
        }
        let sources = if any_direction {
            vec![source, source.other()]
        } else {
            vec![source]
        };
        for from in sources {
            if k.word_pairs(term, from)?.is_some() {
                cx.report("target", Value::Language(from.other().code()));
                return lexicon(cx, k, term, from, lang, false);
            }
        }
        Ok(SkillResult::Unable(
            Message::new("unknown").var("term", term.to_owned()),
        ))
    }

    /// The word-pair lexicon: its first candidate and the second as "also",
    /// always with the hedge; `no_word` when it has nothing either.
    fn lexicon(
        cx: &mut Context<'_>,
        k: &Knowledge,
        term: &str,
        source: Lang,
        lang: Lang,
        known: bool,
    ) -> Result<SkillResult, Error> {
        let Some(pairs) = k.word_pairs(term, source)? else {
            let message = if known {
                Message::new("no_word")
                    .var("term", term.to_owned())
                    .var("target_name", name(cx, lang, source.other()))
            } else {
                Message::new("unknown").var("term", term.to_owned())
            };
            return Ok(SkillResult::Unable(message));
        };
        cx.mark_fallback(Fallback::WORD_PAIRS);
        let first = quoted(cx, lang, &pairs.translations[0]);
        Ok(SkillResult::Answer(match pairs.translations.get(1) {
            Some(second) => Message::new("lexicon_also")
                .var("first", first)
                .var("second", quoted(cx, lang, second)),
            None => Message::new("lexicon").var("first", first),
        }))
    }

    /// A phrase word by word, each word through its concept (its sense
    /// chosen with the rest of the phrase as context, else its first) or,
    /// failing that, the lexicon. `None` when a word is unknown.
    pub(super) fn word_by_word(
        cx: &Context<'_>,
        k: &Knowledge,
        phrase: &str,
        source: Lang,
    ) -> Result<Option<String>, Error> {
        let tokens: Vec<String> = phrase
            .split_whitespace()
            .map(crate::dict::unquote)
            .filter(|t| !t.is_empty())
            .collect();
        let into = source.other();
        let mut out: Vec<String> = Vec::new();
        let mut i = 0;
        'words: while i < tokens.len() {
            for n in (1..=(tokens.len() - i).min(4)).rev() {
                let segment = tokens[i..i + n].join(" ");
                let senses = k.senses_in(&segment, source)?;
                if senses.is_empty() {
                    continue;
                }
                let choice = choose(cx, k, &senses, source, phrase)?;
                let first = choice.chosen.map_or(0, |(c, _)| c);
                let order = std::iter::once(first).chain((0..senses.len()).filter(|c| *c != first));
                for c in order {
                    if let Some(word) = targets(k, &senses[c], into)?.into_iter().next() {
                        out.push(word);
                        i += n;
                        continue 'words;
                    }
                }
            }
            match k.word_pairs(&tokens[i], source)? {
                Some(pairs) => out.push(pairs.translations[0].clone()),
                None => return Ok(None),
            }
            i += 1;
        }
        Ok((out.len() > 1).then(|| out.join(" ")))
    }

    /// A stock phrase: its counterparts in the other language, each with
    /// the note that tells it apart, or the nearest match.
    fn phrase(
        cx: &mut Context<'_>,
        phrases: &[StockPhrase],
        lang: Lang,
        target: Option<Lang>,
    ) -> SkillResult {
        let from = phrases[0].matched.lang;
        let into = target.filter(|t| *t != from).unwrap_or(from.other());
        cx.report("target", Value::Language(into.code()));
        let approximate = phrases.iter().any(|p| p.approximate);
        let mut items = Vec::new();
        for entry in phrases {
            for wording in entry.side(into) {
                let note = wording
                    .note(lang)
                    .or_else(|| (phrases.len() > 1).then(|| entry.matched.note(lang)).flatten());
                let opens = items.is_empty() && !approximate;
                let text = if opens {
                    capitalise(&wording.text)
                } else {
                    wording.text.clone()
                };
                let shown = quoted(cx, lang, &text);
                items.push(match note {
                    Some(note) => p(cx, lang, "p_noted", &[("text", &shown), ("note", note)]),
                    None => shown,
                });
            }
        }
        if approximate && items.len() == 1 {
            return SkillResult::Answer(
                Message::new("phrase_approx")
                    .var("target_name", name(cx, lang, into))
                    .var("text", items.remove(0)),
            );
        }
        SkillResult::Answer(Message::new("phrase").var("text", items.join("; ")))
    }
}

#[cfg(all(test, feature = "knowledge"))]
mod tests {
    use talos_core::{Context, Knowledge, Lang, Random, Timestamp};

    const SEED: &str = include_str!("../../../knowledge/seed.toml");

    #[test]
    fn a_phrase_of_known_words_goes_word_by_word() {
        let k = Knowledge::from_source(SEED).unwrap();
        let cx = Context::standalone(Timestamp::from_unix(0), Random::seeded(0)).with_knowledge(&k);
        let wbw = |t: &str, l| super::imp::word_by_word(&cx, &k, t, l).unwrap();
        assert_eq!(
            wbw("guitarra eléctrica pandereta", Lang::Es).as_deref(),
            Some("electric guitar tambourine")
        );
        assert_eq!(
            wbw("cuerda guitarra", Lang::Es).as_deref(),
            Some("string guitar")
        );
        assert_eq!(wbw("guitarra roja", Lang::Es), None);
        assert_eq!(wbw("violins drums", Lang::En).as_deref(), Some("violines tambores"));
    }
}
