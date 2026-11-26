//! `define`: what a word means, from the knowledge store
//! (docs/spec/knowledge.md, `define.word`; sections 2.1, 2.2, 2.7, 2.8).
//!
//! The word is resolved through its forms, in the conversation's language
//! first and then the other (`Knowledge::lookup`); a known lemma one edit
//! away is asked about, never assumed. Among several senses the choice is
//! the specification's: words of the turn first (they replace a sense
//! remembered earlier), then the remembered sense, recent concepts and the
//! baseline rank; a tie is asked as a choice. The answer is the gloss in
//! the conversation's language. Without one Talos says so and falls back
//! to what the word's claims say (its class, what it is part of), marked
//! `relations`; the other language's gloss is given only when asked for,
//! marked `other-language`.
//!
//! A dictionary pack (a whole dictionary of one language, made from an
//! e-reader dictionary by `talos-pack kobo`) is asked first, when the
//! definition is wanted in the conversation's language: its entry is given
//! as the dictionary has it, the first definitions of each part of speech,
//! with the dictionary named. Nothing is chosen or asked: an entry is not a
//! concept. The steps above are for a word no dictionary pack has.
//!
//! Without the `knowledge` feature, or without a store, every request is
//! answered with "no dictionary" (`Unhandled`).

use talos_core::{Call, Context, Message, Skill, SkillResult};

/// The `define` handler.
#[derive(Debug, Default)]
pub(crate) struct Define;

/// Every reply id the handler can return or compose from.
const REPLIES: &[&str] = &[
    "defined",
    "undefined",
    "unknown",
    "no_dictionary",
    "ask_word",
    "ask_near",
    "ask_sense",
    "ask_sense_foreign",
    "other_gloss",
    "other_gloss_none",
    "near_refused",
    // Pieces.
    "p_gloss",
    "p_gloss_named",
    "p_foreign",
    "p_foreign_unnamed",
    "p_others",
    "p_nodef",
    "p_nodef_any",
    "p_knows_class",
    "p_knows_part",
    "p_said_other",
    "p_said_here",
    "p_other_def",
    "word_or",
    // Pieces of a dictionary pack's entry.
    "p_entry",
    "p_entry_bare",
    "p_entry_more",
    "p_entry_also",
    "p_entry_form",
    "p_entry_form_also",
    "p_entry_source",
    "word_and",
];

impl Skill for Define {
    fn id(&self) -> &'static str {
        "define"
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            // "what does that proverb mean?" points at something said
            // before, which Talos cannot see: not a word to define.
            "define.word" if points(call) => SkillResult::Decline,
            "define.word" => word(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

/// Whether the word captured only points at something ("ese refrán",
/// "that proverb"); a bare "esto" or "that" is asked about instead.
fn points(call: &Call<'_>) -> bool {
    crate::dict::text(call.slots, "word").is_some_and(|w| {
        crate::dict::clean_word(w).is_none() && w.split_whitespace().count() > 1
    })
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
    imp::define(cx, k, call).unwrap_or_else(|e| SkillResult::Fail(talos_core::SkillError(e.to_string())))
}

#[cfg(feature = "knowledge")]
pub(crate) mod imp {
    use talos_core::knowledge::{ConceptId, Cues, Knowledge, Label, Sense, SenseChoice, Step, key};
    use talos_core::{
        Call, Choice, Context, Error, Expectation, Fallback, Lang, Message, SkillResult, Value,
    };

    use crate::dict::{capitalise, clean_word, join, language, piece, text, unquote};

    const SKILL: &str = "define";

    fn p(cx: &Context<'_>, lang: Lang, id: &str, vars: &[(&str, &str)]) -> String {
        piece(cx, SKILL, lang, id, vars)
    }

    /// The word asked about and what follows it in the capture: the longest
    /// run of leading words the store knows ("banco" in "banco donde guardo
    /// el dinero"), the rest being context for the sense (2.1, 2.2). A text
    /// with no known start is one unknown word, cut at a comma.
    pub(crate) fn split(k: &Knowledge, text: &str, lang: Lang) -> Result<(String, String), Error> {
        let words: Vec<&str> = text.split_whitespace().collect();
        for n in (1..=words.len().min(6)).rev() {
            let head = unquote(&words[..n].join(" "));
            if !head.is_empty() && k.lookup(&head, lang)?.is_some() {
                return Ok((head, words[n..].join(" ")));
            }
        }
        let (head, rest) = text.split_once(',').unwrap_or((text, ""));
        Ok((unquote(head), rest.trim().to_owned()))
    }

    /// How a sense is named for the user: its class word, or the first
    /// words of its gloss; for a word of the other language, the
    /// concept's own word in the conversation's language when it has one.
    pub(crate) fn label(
        k: &Knowledge,
        concept: &ConceptId,
        label: &Label,
        lang: Lang,
        foreign: bool,
    ) -> Result<String, Error> {
        if foreign && let Some(word) = k.words(concept, lang)?.into_iter().next() {
            return Ok(word.lemma);
        }
        Ok(match label {
            Label::Class { word, .. } => word.lemma.clone(),
            Label::Gloss(gloss) => gloss.clone(),
            Label::None => concept.as_str().to_owned(),
        })
    }

    /// Chooses a sense (2.2): explicit context in the turn decides first
    /// and replaces any sense remembered; then the remembered sense, the
    /// recent concepts and the baseline rank.
    pub(crate) fn choose(
        cx: &Context<'_>,
        k: &Knowledge,
        senses: &[Sense],
        lang: Lang,
        context: &str,
    ) -> Result<SenseChoice, Error> {
        let turn = k.choose_sense(
            senses,
            &Cues {
                context,
                ..Cues::none(lang)
            },
        )?;
        if matches!(turn.chosen, Some((_, Step::Turn))) {
            return Ok(turn);
        }
        let word_key = key(&senses[0].word.lemma);
        let remembered = cx
            .conversation()
            .sense(&word_key)
            .map(ConceptId::new);
        let recent: Vec<ConceptId> = cx
            .conversation()
            .recent_concepts()
            .into_iter()
            .map(ConceptId::new)
            .collect();
        k.choose_sense(
            senses,
            &Cues {
                lang,
                remembered: remembered.as_ref(),
                context,
                recent: &recent,
                relation: None,
            },
        )
    }

    /// The options of "which one?": each sense by its label, answerable by
    /// that label, the concept's words or the words of its glosses, in
    /// either language, or by position.
    pub(crate) fn options(
        k: &Knowledge,
        choice: &SenseChoice,
        lang: Lang,
        foreign: bool,
    ) -> Result<(Vec<String>, Vec<Choice>), Error> {
        let mut labels = Vec::new();
        let mut choices = Vec::new();
        for option in &choice.options {
            let shown = label(k, &option.concept, &option.label, lang, foreign)?;
            let mut names = vec![shown.clone()];
            if let Label::Class { concept, .. } = &option.label {
                for l in Lang::ALL {
                    names.extend(k.words(concept, l)?.into_iter().map(|w| w.lemma));
                }
            }
            for l in Lang::ALL {
                names.extend(k.words(&option.concept, l)?.into_iter().map(|w| w.lemma));
                if let Some(gloss) = k.gloss(&option.concept, l)? {
                    names.push(gloss.text);
                }
            }
            labels.push(shown);
            choices.push(Choice::new(option.concept.as_str(), names));
        }
        Ok((labels, choices))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the steps of docs/spec/knowledge.md 2.1 and 2.2, in order"
    )]
    pub(crate) fn define(
        cx: &mut Context<'_>,
        k: &Knowledge,
        call: &Call<'_>,
    ) -> Result<SkillResult, Error> {
        let lang = call.lang;
        let Ok(gloss_lang) = language(call.slots, "gloss_lang") else {
            return Ok(SkillResult::Unable(Message::new("other_gloss_none")));
        };
        // The answer to "¿te refieres a «guitarra»?".
        let typed = match call.slots.get("word") {
            Some(Value::Confirm(yes)) => {
                let suggested = text(call.slots, "suggested").unwrap_or_default().to_owned();
                let typed = text(call.slots, "typed").unwrap_or_default().to_owned();
                if !yes {
                    cx.report("word", Value::Text(typed.clone()));
                    return Ok(SkillResult::Unable(
                        Message::new("near_refused").var("word", typed),
                    ));
                }
                Some(suggested)
            }
            _ => text(call.slots, "word").and_then(clean_word),
        };
        let Some(raw) = typed.or_else(|| cx.conversation().topic().map(str::to_owned)) else {
            return Ok(SkillResult::Ask(
                Expectation::slot("word"),
                Message::new("ask_word"),
            ));
        };
        if gloss_lang.is_none_or(|l| l == lang)
            && let Some(answer) = entry(cx, k, &raw, lang)?
        {
            return Ok(answer);
        }
        let (word, context) = split(k, &raw, lang)?;
        let Some(found) = k.lookup(&word, lang)? else {
            let near = k.near_misses(&word, lang)?;
            if let [suggestion] = near.as_slice()
                && !word.contains(' ')
            {
                return Ok(SkillResult::Ask(
                    Expectation::confirm("word")
                        .keeping("suggested", Value::Text(suggestion.lemma.clone()))
                        .keeping("typed", Value::Text(word.clone())),
                    Message::new("ask_near").var("word", suggestion.lemma.clone()),
                ));
            }
            cx.report("word", Value::Text(word.clone()));
            return Ok(SkillResult::Unable(Message::new("unknown").var("word", word)));
        };
        let foreign = found.lang != lang;
        let lemma = found.senses[0].word.lemma.clone();
        cx.report("word", Value::Text(lemma.clone()));
        // A sense chosen by the user in answer to "which one?".
        let asked = text(call.slots, "sense")
            .and_then(|id| found.senses.iter().position(|s| s.concept.as_str() == id));
        let choice = choose(cx, k, &found.senses, lang, &context)?;
        let chosen = match asked {
            Some(i) => Some((i, Step::Remembered)),
            None => choice.chosen,
        };
        let Some((index, step)) = chosen else {
            let (labels, choices) = options(k, &choice, lang, foreign)?;
            let or = p(cx, lang, "word_or", &[]);
            let message = if foreign {
                Message::new("ask_sense_foreign").var("word", capitalise(&lemma))
            } else {
                Message::new("ask_sense").var("word", lemma)
            };
            return Ok(SkillResult::Ask(
                Expectation::choice("sense", choices),
                message.var("options", join(&labels, &or)),
            ));
        };
        let concept = found.senses[index].concept.clone();
        if asked.is_some() || step == Step::Turn {
            cx.remember_sense(key(&lemma), concept.as_str());
        }
        cx.mention(concept.as_str());
        cx.set_topic(lemma.clone());
        cx.report("sense", Value::Text(concept.as_str().to_owned()));

        let own = k.gloss(&concept, lang)?;
        if let Some(asked_lang) = gloss_lang.filter(|l| *l != lang) {
            cx.report("gloss_lang", Value::Language(asked_lang.code()));
            return Ok(match k.gloss(&concept, asked_lang)? {
                Some(gloss) => {
                    if own.is_none() {
                        cx.mark_fallback(Fallback::OTHER_LANGUAGE);
                    }
                    SkillResult::Answer(Message::new("other_gloss").var("gloss", gloss.text))
                }
                None => SkillResult::Unable(Message::new("other_gloss_none").var("word", lemma)),
            });
        }
        let Some(gloss) = own else {
            return undefined(cx, k, &concept, &lemma, lang, foreign);
        };
        let named = match (&choice.options[index].label, step) {
            (Label::Class { word, .. }, Step::Turn | Step::Recent) if found.senses.len() > 1 => {
                Some(word.lemma.clone())
            }
            _ => None,
        };
        let name = if foreign {
            k.words(&concept, lang)?.into_iter().next().map(|w| w.lemma)
        } else {
            None
        };
        let mut text = gloss_text(cx, lang, &lemma, &gloss.text, foreign, name, named);
        if step == Step::Rank && asked.is_none() && found.senses.len() > 1 {
            let mut others = Vec::new();
            for (i, option) in choice.options.iter().enumerate() {
                if i != index {
                    others.push(label(k, &option.concept, &option.label, lang, foreign)?);
                }
            }
            let or = p(cx, lang, "word_or", &[]);
            text.push(' ');
            text.push_str(&p(cx, lang, "p_others", &[("others", &join(&others, &or))]));
        }
        Ok(SkillResult::Answer(Message::new("defined").var("text", text)))
    }

    /// How many definitions of a part of speech, and how many parts of
    /// speech, an answer gives: an entry may have forty definitions.
    const DEFINITIONS: usize = 3;
    const SECTIONS: usize = 2;

    /// The word's entry in a dictionary pack, as an answer: the longest run
    /// of leading words a dictionary has ("perro" in "perro en español"),
    /// each part of speech on a line with its first definitions, and the
    /// dictionary's name. `None` when no dictionary pack has the word.
    fn entry(
        cx: &mut Context<'_>,
        k: &Knowledge,
        raw: &str,
        lang: Lang,
    ) -> Result<Option<SkillResult>, Error> {
        let words: Vec<&str> = raw.split_whitespace().collect();
        let mut found = None;
        for n in (1..=words.len().min(6)).rev() {
            let head = unquote(&words[..n].join(" "));
            if head.is_empty() {
                continue;
            }
            let entries = k.entries(&head, lang)?;
            if !entries.is_empty() {
                found = Some((head, entries));
                break;
            }
        }
        let Some((typed, entries)) = found else {
            return Ok(None);
        };
        let first = &entries[0];
        let and = p(cx, lang, "word_and", &[]);
        let mut lines = Vec::new();
        if first.inflected {
            let others: Vec<String> = entries[1..]
                .iter()
                .map(|e| format!("«{}»", e.lemma))
                .collect();
            let vars = [
                ("form", typed.as_str()),
                ("word", first.lemma.as_str()),
                ("others", &join(&others, &and)),
            ];
            let id = if others.is_empty() {
                "p_entry_form"
            } else {
                "p_entry_form_also"
            };
            lines.push(p(cx, lang, id, &vars));
        }
        let word = capitalise(&first.lemma);
        for section in first.sections.iter().take(SECTIONS) {
            let shown = &section.definitions[..section.definitions.len().min(DEFINITIONS)];
            let mut definitions = if let [only] = shown {
                closed(only)
            } else {
                shown
                    .iter()
                    .enumerate()
                    .map(|(i, d)| format!("{}. {}", i + 1, closed(d)))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let more = section.definitions.len() - shown.len();
            if more > 0 {
                definitions.push(' ');
                definitions.push_str(&p(cx, lang, "p_entry_more", &[("n", &more.to_string())]));
            }
            lines.push(match &section.pos {
                Some(pos) => p(
                    cx,
                    lang,
                    "p_entry",
                    &[("word", &word), ("pos", pos), ("definitions", &definitions)],
                ),
                None => p(
                    cx,
                    lang,
                    "p_entry_bare",
                    &[("word", &word), ("definitions", &definitions)],
                ),
            });
        }
        let others: Vec<String> = first
            .sections
            .iter()
            .skip(SECTIONS)
            .filter_map(|s| s.pos.clone())
            .collect();
        if !others.is_empty() {
            lines.push(p(cx, lang, "p_entry_also", &[("others", &join(&others, &and))]));
        }
        lines.push(p(cx, lang, "p_entry_source", &[("name", &first.source.name)]));
        cx.report("word", Value::Text(first.lemma.clone()));
        cx.set_topic(first.lemma.clone());
        Ok(Some(SkillResult::Answer(
            Message::new("defined").var("text", lines.join("\n")),
        )))
    }

    /// A definition ending as a sentence does.
    fn closed(definition: &str) -> String {
        if definition.ends_with(['.', '!', '?', '…', ')']) {
            definition.to_owned()
        } else {
            format!("{definition}.")
        }
    }

    /// The definition: "Word: gloss.", "Word (class): gloss." when context
    /// chose the sense, or, for a word of the other language, the word,
    /// its name in this language and the gloss.
    fn gloss_text(
        cx: &Context<'_>,
        lang: Lang,
        lemma: &str,
        gloss: &str,
        foreign: bool,
        name: Option<String>,
        named: Option<String>,
    ) -> String {
        let word = capitalise(lemma);
        match (foreign, name, named) {
            (true, Some(name), _) => p(
                cx,
                lang,
                "p_foreign",
                &[("word", &word), ("name", &name), ("gloss", gloss)],
            ),
            (true, None, _) => p(
                cx,
                lang,
                "p_foreign_unnamed",
                &[("word", &word), ("gloss", gloss)],
            ),
            (false, _, Some(label)) => p(
                cx,
                lang,
                "p_gloss_named",
                &[("word", &word), ("label", &label), ("gloss", gloss)],
            ),
            (false, _, None) => p(cx, lang, "p_gloss", &[("word", &word), ("gloss", gloss)]),
        }
    }

    /// No definition in the conversation's language: say so, then what the
    /// claims say (marked `relations`), and the word in the other language.
    fn undefined(
        cx: &mut Context<'_>,
        k: &Knowledge,
        concept: &ConceptId,
        lemma: &str,
        lang: Lang,
        foreign: bool,
    ) -> Result<SkillResult, Error> {
        let other_gloss = k.gloss(concept, lang.other())?.is_some();
        let mut parts = vec![if other_gloss || foreign {
            p(cx, lang, "p_nodef", &[("word", lemma)])
        } else {
            p(cx, lang, "p_nodef_any", &[("word", lemma)])
        }];
        let mut related = None;
        for (predicate, id) in [("is_a", "p_knows_class"), ("part_of", "p_knows_part")] {
            for object in k.objects(concept, predicate)? {
                if let Some(name) = k.name(&object, lang)? {
                    related = Some((id, name.lemma));
                    break;
                }
            }
            if related.is_some() {
                break;
            }
        }
        if let Some((id, name)) = &related {
            parts.push(p(cx, lang, id, &[("class", name)]));
            cx.mark_fallback(Fallback::RELATIONS);
        }
        let (said, other) = if foreign {
            ("p_said_here", k.words(concept, lang)?.into_iter().next())
        } else {
            ("p_said_other", k.words(concept, lang.other())?.into_iter().next())
        };
        if let Some(other) = other {
            parts.push(p(cx, lang, said, &[("other", &other.lemma)]));
        }
        if related.is_none() && other_gloss {
            parts.push(p(cx, lang, "p_other_def", &[]));
        }
        Ok(SkillResult::Unable(
            Message::new("undefined").var("text", parts.join(" ")),
        ))
    }
}

#[cfg(all(test, feature = "knowledge"))]
mod tests {
    use super::imp::split;
    use talos_core::{Knowledge, Lang};

    const SEED: &str = include_str!("../../../knowledge/seed.toml");

    #[test]
    fn the_longest_known_start_is_the_word() {
        let k = Knowledge::from_source(SEED).unwrap();
        let split = |t: &str, l| split(&k, t, l).unwrap();
        assert_eq!(
            split("banco donde guardo el dinero", Lang::Es),
            ("banco".to_owned(), "donde guardo el dinero".to_owned())
        );
        assert_eq!(
            split("caja de resonancia", Lang::Es),
            ("caja de resonancia".to_owned(), String::new())
        );
        assert_eq!(
            split("bank, as in the side of a river", Lang::En),
            ("bank".to_owned(), "as in the side of a river".to_owned())
        );
        assert_eq!(
            split("física cuántica", Lang::Es),
            ("física cuántica".to_owned(), String::new())
        );
    }
}
