//! `teach`: the user's own phrases (docs/spec/knowledge.md, `teach.phrase`,
//! and the phrase parts of `teach.list` and `teach.forget`). Teaching facts
//! is v0.6 and not here.
//!
//! "Cuando diga «tira», quiero decir «tira un dado»": the meaning is read as
//! a message would be ([`Context::understand`]); a request Talos
//! understands is stored in the user-correction table (ADR 0008), which the
//! engine consults at the rule stage, so the phrase alone is that request
//! from then on. A meaning that deletes anything is refused, one that is
//! not a request is refused, and a phrase that already means something
//! else is only replaced after a yes. These intents change what is stored:
//! on a classifier's guess the handler restates the change and asks first
//! (DESIGN.md section 11). Nothing here needs the knowledge store.

use talos_core::normalise::normalise;
use talos_core::{
    Call, Context, Correction, Expectation, Lang, Message, NOT_TEACHABLE, Number, Recognition,
    Skill, SkillResult, Value,
};

use crate::dict::{join, piece, strip_quotes, text};

/// The `teach` handler.
#[derive(Debug, Default)]
pub(crate) struct Teach;

const SKILL: &str = "teach";

/// How many taught phrases a list shows; the rest are counted.
const LIST_MAX: usize = 10;

/// How a stored meaning is described, per intent: (`will_*` reply,
/// `doing_*` reply, the slots it needs), the first whose slots are all
/// there winning. `will` says what Talos will do ("tiraré un dado"),
/// `doing` names it ("tirar un dado").
type Wording = (&'static str, &'static str, &'static [&'static str]);

const MEANINGS: &[(&str, &[Wording])] = &[
    ("smalltalk.greet", &[("will_greet", "doing_greet", &[])]),
    ("smalltalk.bye", &[("will_bye", "doing_bye", &[])]),
    ("smalltalk.thanks", &[("will_thanks", "doing_thanks", &[])]),
    ("smalltalk.how_are_you", &[("will_how_are_you", "doing_how_are_you", &[])]),
    ("identity.who", &[("will_who", "doing_who", &[])]),
    ("identity.nature", &[("will_nature", "doing_nature", &[])]),
    ("help.capabilities", &[("will_capabilities", "doing_capabilities", &[])]),
    (
        "help.skill",
        &[
            ("will_skill_help", "doing_skill_help", &["skill"]),
            ("will_help", "doing_help", &[]),
        ],
    ),
    ("clock.now", &[("will_now", "doing_now", &[])]),
    (
        "clock.date",
        &[
            ("will_date_day", "doing_date_day", &["day"]),
            ("will_date", "doing_date", &[]),
        ],
    ),
    ("clock.weekday", &[("will_weekday", "doing_weekday", &[])]),
    (
        "clock.until",
        &[
            ("will_until_target", "doing_until_target", &["target"]),
            ("will_until", "doing_until", &[]),
        ],
    ),
    (
        "timezones.time_in",
        &[
            ("will_time_in_place", "doing_time_in_place", &["place"]),
            ("will_time_in", "doing_time_in", &[]),
        ],
    ),
    ("timezones.difference", &[("will_difference", "doing_difference", &[])]),
    (
        "calc.arithmetic",
        &[
            ("will_arithmetic_expression", "doing_arithmetic_expression", &["expression"]),
            ("will_arithmetic", "doing_arithmetic", &[]),
        ],
    ),
    ("calc.percent", &[("will_percent", "doing_percent", &[])]),
    ("calc.convert", &[("will_convert", "doing_convert", &[])]),
    (
        "random.dice",
        &[
            ("will_dice_count_sides", "doing_dice_count_sides", &["count", "sides"]),
            ("will_dice_count", "doing_dice_count", &["count"]),
            ("will_dice", "doing_dice", &[]),
        ],
    ),
    ("random.coin", &[("will_coin", "doing_coin", &[])]),
    (
        "random.pick",
        &[
            ("will_pick_options", "doing_pick_options", &["options"]),
            ("will_pick", "doing_pick", &[]),
        ],
    ),
    (
        "random.number",
        &[
            ("will_number_range", "doing_number_range", &["low", "high"]),
            ("will_number", "doing_number", &[]),
        ],
    ),
    ("fortune.tell", &[("will_fortune", "doing_fortune", &[])]),
    (
        "preferences.set",
        &[
            ("will_set_value", "doing_set_value", &["value"]),
            ("will_set", "doing_set", &[]),
        ],
    ),
    ("preferences.get", &[("will_get", "doing_get", &[])]),
    (
        "define.word",
        &[
            ("will_define_word", "doing_define_word", &["word"]),
            ("will_define", "doing_define", &[]),
        ],
    ),
    (
        "translate.word",
        &[
            ("will_translate_term", "doing_translate_term", &["term"]),
            ("will_translate", "doing_translate", &[]),
        ],
    ),
    ("teach.list", &[("will_list", "doing_list", &[])]),
];

/// Every reply id the handler can return or compose from.
const REPLIES: &[&str] = &[
    "phrase_taught",
    "phrase_replaced",
    "phrase_kept",
    "phrase_unknown",
    "phrase_thing",
    "phrase_deletes",
    "phrase_ask_phrase",
    "phrase_ask_meaning",
    "phrase_confirm_replace",
    "phrase_confirm_new",
    "list_none",
    "list_one",
    "list_many",
    "list_more",
    "list_facts",
    "list_topic",
    "forget_fact",
    "forget_done",
    "forget_none",
    "forget_ask",
    "forget_confirm_one",
    "forget_all_confirm",
    "forget_all_done",
    "forget_all_none",
    "forget_kept",
    // Pieces.
    "p_list_item",
    "word_and",
    "will_other",
    "doing_other",
    "will_greet",
    "doing_greet",
    "will_bye",
    "doing_bye",
    "will_thanks",
    "doing_thanks",
    "will_how_are_you",
    "doing_how_are_you",
    "will_who",
    "doing_who",
    "will_nature",
    "doing_nature",
    "will_capabilities",
    "doing_capabilities",
    "will_skill_help",
    "doing_skill_help",
    "will_help",
    "doing_help",
    "will_now",
    "doing_now",
    "will_date_day",
    "doing_date_day",
    "will_date",
    "doing_date",
    "will_weekday",
    "doing_weekday",
    "will_until_target",
    "doing_until_target",
    "will_until",
    "doing_until",
    "will_time_in_place",
    "doing_time_in_place",
    "will_time_in",
    "doing_time_in",
    "will_difference",
    "doing_difference",
    "will_arithmetic_expression",
    "doing_arithmetic_expression",
    "will_arithmetic",
    "doing_arithmetic",
    "will_percent",
    "doing_percent",
    "will_convert",
    "doing_convert",
    "will_dice_count_sides",
    "doing_dice_count_sides",
    "will_dice_count",
    "doing_dice_count",
    "will_dice",
    "doing_dice",
    "will_coin",
    "doing_coin",
    "will_pick_options",
    "doing_pick_options",
    "will_pick",
    "doing_pick",
    "will_number_range",
    "doing_number_range",
    "will_number",
    "doing_number",
    "will_fortune",
    "doing_fortune",
    "will_set_value",
    "doing_set_value",
    "will_set",
    "doing_set",
    "will_get",
    "doing_get",
    "will_define_word",
    "doing_define_word",
    "will_define",
    "doing_define",
    "will_translate_term",
    "doing_translate_term",
    "will_translate",
    "doing_translate",
    "will_list",
    "doing_list",
];

impl Skill for Teach {
    fn id(&self) -> &'static str {
        "teach"
    }

    fn replies(&self) -> &'static [&'static str] {
        REPLIES
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "teach.phrase" => phrase(cx, call),
            "teach.list" => list(cx, call),
            "teach.forget" => forget(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

/// Which description of a meaning.
#[derive(Clone, Copy)]
enum Form {
    /// What Talos will do: "tiraré un dado".
    Will,
    /// The action named: "tirar un dado".
    Doing,
}

/// Describes what a stored meaning does, from the manifest's wording.
fn describe(
    cx: &Context<'_>,
    lang: Lang,
    intent: &str,
    slots: &std::collections::BTreeMap<String, String>,
    form: Form,
) -> String {
    let pick = |w: &Wording| match form {
        Form::Will => w.0,
        Form::Doing => w.1,
    };
    if let Some((_, wordings)) = MEANINGS.iter().find(|(id, _)| *id == intent)
        && let Some(wording) = wordings
            .iter()
            .find(|w| w.2.iter().all(|s| slots.get(*s).is_some_and(|v| !v.is_empty())))
    {
        let vars: Vec<(&str, &str)> = wording
            .2
            .iter()
            .filter_map(|s| slots.get(*s).map(|v| (*s, v.as_str())))
            .collect();
        return piece(cx, SKILL, lang, pick(wording), &vars);
    }
    let id = match form {
        Form::Will => "will_other",
        Form::Doing => "doing_other",
    };
    piece(cx, SKILL, lang, id, &[("intent", intent)])
}

fn confirm(call: &Call<'_>) -> Option<bool> {
    call.slots.get("confirm").and_then(Value::as_confirm)
}

/// Whether a meaning names a thing rather than asking for something: "la
/// guitarra", "the guitar".
fn is_thing(meaning: &str) -> bool {
    const ARTICLES: &[&str] = &[
        "el", "la", "los", "las", "un", "una", "unos", "unas", "the", "a", "an", "mi", "my",
    ];
    let folded = normalise(meaning).folded;
    let words: Vec<&str> = folded.split_whitespace().collect();
    words.len() <= 4 && words.first().is_some_and(|w| ARTICLES.contains(w))
}

/// The slots of a stored meaning, for the outcome: `intent slot=value …`.
fn meaning_line(intent: &str, slots: &std::collections::BTreeMap<String, String>) -> String {
    let mut line = intent.to_owned();
    for (name, value) in slots {
        line.push(' ');
        line.push_str(name);
        line.push('=');
        line.push_str(value);
    }
    line
}

fn phrase(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let lang = call.lang;
    let Some(phrase) = text(call.slots, "phrase").map(strip_quotes).filter(|p| !p.is_empty())
    else {
        return SkillResult::Ask(Expectation::slot("phrase"), Message::new("phrase_ask_phrase"));
    };
    let key = Correction::key(&phrase);
    if key.is_empty() {
        return SkillResult::Ask(Expectation::slot("phrase"), Message::new("phrase_ask_phrase"));
    }
    cx.report("phrase", Value::Text(phrase.clone()));
    let Some(meaning) = text(call.slots, "meaning").map(strip_quotes).filter(|m| !m.is_empty())
    else {
        return SkillResult::Ask(
            Expectation::text("meaning"),
            Message::new("phrase_ask_meaning"),
        );
    };
    cx.report("meaning", Value::Text(meaning.clone()));
    if confirm(call) == Some(false) {
        return SkillResult::Answer(Message::new("phrase_kept"));
    }
    let Some(understood) = cx.understand(&meaning, lang) else {
        let id = if is_thing(&meaning) {
            "phrase_thing"
        } else {
            "phrase_unknown"
        };
        return SkillResult::Unable(Message::new(id).var("meaning", meaning));
    };
    let intent = understood.intent.as_str().to_owned();
    if NOT_TEACHABLE.contains(&intent.as_str()) {
        return SkillResult::Unable(Message::new("phrase_deletes"));
    }
    cx.report(
        "meaning",
        Value::Text(meaning_line(&intent, &understood.slots)),
    );
    // What the phrase means now: an earlier mapping (replaced unless it is
    // the same request), or a rule (shadowed unless it is the same intent:
    // "¿y hoy?" may already ask for the date).
    let existing = cx
        .corrections()
        .iter()
        .rev()
        .find(|c| c.phrase == key)
        .map(|c| (c.intent.as_str().to_owned(), c.slots.clone(), true))
        .or_else(|| {
            cx.understand(&phrase, lang)
                .filter(|u| u.source != Recognition::Classifier)
                .map(|u| (u.intent.as_str().to_owned(), u.slots, false))
        });
    let replacing = existing.as_ref().is_some_and(|(i, s, taught)| {
        *i != intent || (*taught && *s != understood.slots)
    });
    if confirm(call) != Some(true) {
        if let Some((old_intent, old_slots, _)) = existing.as_ref().filter(|_| replacing) {
            return SkillResult::Ask(
                Expectation::confirm("confirm"),
                Message::new("phrase_confirm_replace")
                    .var("phrase", crate::dict::capitalise(&phrase))
                    .var("old", describe(cx, lang, old_intent, old_slots, Form::Doing))
                    .var(
                        "new",
                        describe(cx, lang, &intent, &understood.slots, Form::Doing),
                    ),
            );
        }
        if cx.guessed() {
            return SkillResult::Ask(
                Expectation::confirm("confirm"),
                Message::new("phrase_confirm_new")
                    .var("phrase", phrase.clone())
                    .var(
                        "doing",
                        describe(cx, lang, &intent, &understood.slots, Form::Doing),
                    ),
            );
        }
    }
    let will = describe(cx, lang, &intent, &understood.slots, Form::Will);
    cx.teach_phrase(Correction::new(&phrase, intent, understood.slots));
    let id = if replacing {
        "phrase_replaced"
    } else {
        "phrase_taught"
    };
    SkillResult::Answer(Message::new(id).var("phrase", phrase).var("will", will))
}

fn list(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let lang = call.lang;
    if text(call.slots, "kind") == Some("facts") {
        cx.report("count", Value::Number(Number::from_int(0)));
        return SkillResult::Answer(Message::new("list_facts"));
    }
    // Only taught facts have a topic, and they are v0.6.
    if let Some(topic) = text(call.slots, "topic").map(strip_quotes) {
        cx.report("count", Value::Number(Number::from_int(0)));
        return SkillResult::Answer(Message::new("list_topic").var("topic", topic));
    }
    let all = cx.corrections();
    let count = all.len();
    cx.report(
        "count",
        Value::Number(Number::from_int(i64::try_from(count).unwrap_or(i64::MAX))),
    );
    if count == 0 {
        return SkillResult::Answer(Message::new("list_none"));
    }
    let shown = &all[count.saturating_sub(LIST_MAX)..];
    let items: Vec<String> = shown
        .iter()
        .map(|c| {
            let doing = describe(cx, lang, c.intent.as_str(), &c.slots, Form::Doing);
            piece(
                cx,
                SKILL,
                lang,
                "p_list_item",
                &[("phrase", &c.phrase), ("doing", &doing)],
            )
        })
        .collect();
    let and = piece(cx, SKILL, lang, "word_and", &[]);
    let items = join(&items, &and);
    let number = |n: usize| Number::from_int(i64::try_from(n).unwrap_or(i64::MAX)).format(lang);
    SkillResult::Answer(match count {
        1 => Message::new("list_one").var("items", items),
        n if n <= LIST_MAX => Message::new("list_many")
            .var("count", number(n))
            .var("items", items),
        n => Message::new("list_more")
            .var("count", number(n))
            .var("shown", number(LIST_MAX))
            .var("more", number(n - LIST_MAX))
            .var("items", items),
    })
}

fn forget(cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
    let lang = call.lang;
    let Some(target) = text(call.slots, "target").map(strip_quotes).filter(|t| !t.is_empty())
    else {
        return SkillResult::Ask(Expectation::slot("target"), Message::new("forget_ask"));
    };
    cx.report("target", Value::Text(target.clone()));
    let answer = confirm(call);
    if answer == Some(false) {
        return SkillResult::Answer(Message::new("forget_kept"));
    }
    if target == "all" {
        let all: Vec<Correction> = cx.corrections().to_vec();
        let count = all.len();
        let shown = Number::from_int(i64::try_from(count).unwrap_or(i64::MAX)).format(lang);
        if count == 0 {
            return SkillResult::Answer(Message::new("forget_all_none"));
        }
        if (count > 1 || cx.guessed()) && answer != Some(true) {
            return SkillResult::Ask(
                Expectation::confirm("confirm"),
                Message::new("forget_all_confirm").var("count", shown),
            );
        }
        for c in &all {
            cx.forget_phrase(c.phrase.clone());
        }
        cx.report(
            "count",
            Value::Number(Number::from_int(i64::try_from(count).unwrap_or(i64::MAX))),
        );
        if let [only] = all.as_slice() {
            let doing = describe(cx, lang, only.intent.as_str(), &only.slots, Form::Doing);
            return SkillResult::Answer(
                Message::new("forget_done")
                    .var("phrase", only.phrase.clone())
                    .var("doing", doing),
            );
        }
        return SkillResult::Answer(Message::new("forget_all_done").var("count", shown));
    }
    // The claim of the last answer is a fact; facts are v0.6.
    if target == "that" {
        return SkillResult::Unable(Message::new("forget_fact"));
    }
    let found = if target == "last" {
        let Some(last) = cx.corrections().last().cloned() else {
            return SkillResult::Answer(Message::new("forget_all_none"));
        };
        last
    } else {
        let key = Correction::key(&target);
        let Some(found) = cx.corrections().iter().rev().find(|c| c.phrase == key).cloned() else {
            if text(call.slots, "shape") == Some("fact") {
                return SkillResult::Unable(Message::new("forget_fact"));
            }
            return SkillResult::Answer(Message::new("forget_none").var("phrase", target));
        };
        found
    };
    let (key, target) = (found.phrase.clone(), found.phrase.clone());
    let doing = describe(cx, lang, found.intent.as_str(), &found.slots, Form::Doing);
    if cx.guessed() && answer != Some(true) {
        return SkillResult::Ask(
            Expectation::confirm("confirm"),
            Message::new("forget_confirm_one")
                .var("phrase", target)
                .var("doing", doing),
        );
    }
    cx.forget_phrase(key);
    SkillResult::Answer(
        Message::new("forget_done")
            .var("phrase", target)
            .var("doing", doing),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_meaning_wording_is_a_declared_reply() {
        for (_, wordings) in MEANINGS {
            for (will, doing, _) in *wordings {
                assert!(REPLIES.contains(will), "{will}");
                assert!(REPLIES.contains(doing), "{doing}");
            }
        }
    }

    #[test]
    fn things_are_told_from_requests() {
        assert!(is_thing("la guitarra"));
        assert!(is_thing("the guitar"));
        assert!(!is_thing("haz magia"));
        assert!(!is_thing("tira un dado"));
    }
}
