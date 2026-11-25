//! `talos eval`: runs the batteries of `evals/` (format in
//! `evals/README.md`) through two engines, rules only and rules plus the
//! classifier, and reports per language: intent accuracy over all rows and
//! over the rows of built skills, wrong intents apart from missed ones, the
//! false-accept rate of the rejection set by kind, the catch-all rate of
//! the conversation battery, and accuracy per intent. A report, not a
//! gate: the command exits 0 whatever the scores.
//!
//! Every intent and reject row is a fresh single-turn conversation with the
//! row's language as the caller's hint; a conversation runs its turns in
//! one conversation, also with the hint. Only counts are printed, never a
//! phrase: the batteries are a test set (evals/README.md, independence
//! rule), and nothing here may help copy them into a manifest.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value as Json, json};
use talos_core::{ConvId, Engine, Lang, Reason, Status, Timestamp, Turn};

/// The languages of the batteries, in report order.
const LANGS: [Lang; 2] = [Lang::Es, Lang::En];

/// One battery row: an intent row has `intent`, a reject row has `status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) lang: Lang,
    /// The label of an intent row.
    pub(crate) intent: Option<String>,
    /// The `kind` of a reject row.
    pub(crate) kind: Option<String>,
    /// The status a reject row must get.
    pub(crate) status: Option<String>,
    /// Tagged `in-spec`: the text also appears in the specification, so
    /// manifest authors have seen it and the row is not held out.
    pub(crate) in_spec: bool,
}

/// What the engine did with a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observed {
    pub(crate) status: Status,
    pub(crate) intent: Option<String>,
}

/// Rows seen and rows that hit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Count {
    pub(crate) rows: usize,
    pub(crate) hits: usize,
}

/// Intent accuracy of one battery.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct IntentScore {
    pub(crate) total: Count,
    /// Rows recognised as another intent.
    pub(crate) confused: usize,
    /// Rows recognised as no intent.
    pub(crate) missed: usize,
    pub(crate) per_intent: BTreeMap<String, Count>,
    /// The same, over the rows not tagged `in-spec`.
    pub(crate) held_out: BTreeMap<String, Count>,
}

impl IntentScore {
    /// The held-out rows whose label belongs to one of `skills`.
    pub(crate) fn built_held_out(&self, skills: &BTreeSet<String>) -> Count {
        let mut count = Count::default();
        for (intent, c) in &self.held_out {
            if skills.contains(intent.split('.').next().unwrap_or_default()) {
                count.rows += c.rows;
                count.hits += c.hits;
            }
        }
        count
    }

    /// The rows whose label belongs to one of `skills`.
    pub(crate) fn built(&self, skills: &BTreeSet<String>) -> Count {
        let mut count = Count::default();
        for (intent, c) in &self.per_intent {
            if skills.contains(intent.split('.').next().unwrap_or_default()) {
                count.rows += c.rows;
                count.hits += c.hits;
            }
        }
        count
    }
}

/// The rejection set of one language.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct RejectScore {
    pub(crate) rows: usize,
    /// Rows answered with an intent.
    pub(crate) false_accepts: usize,
    /// Rows whose status is the one the file expects.
    pub(crate) status_correct: usize,
    /// Per `kind`: rows and false accepts (hits).
    pub(crate) by_kind: BTreeMap<String, Count>,
}

/// The conversation battery of one language.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConversationScore {
    /// Conversations run (those with a `given` are skipped).
    pub(crate) conversations: usize,
    pub(crate) turns: usize,
    /// Turns ending in the bare catch-all (`unhandled · catch-all`).
    pub(crate) catch_all: usize,
    /// Turns ending `unhandled` that the battery did not expect to.
    pub(crate) unexpected_unhandled: usize,
    /// Turns whose status is the one the battery expects.
    pub(crate) status_correct: usize,
}

/// What one engine scored on one language.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Scores {
    pub(crate) intents: IntentScore,
    pub(crate) reject: RejectScore,
    /// `None` when the language has no conversation battery.
    pub(crate) conversations: Option<ConversationScore>,
}

/// One language: rules only, and rules with the classifier.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LangReport {
    pub(crate) lang: Lang,
    pub(crate) rules: Scores,
    pub(crate) classifier: Scores,
}

/// The whole report.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Report {
    /// The installed skills, whose intents are "built".
    pub(crate) skills: BTreeSet<String>,
    /// The intent model, or why there is none.
    pub(crate) model: String,
    pub(crate) languages: Vec<LangReport>,
}

fn field<'a>(object: &'a Json, name: &str) -> Option<&'a str> {
    object.get(name).and_then(Json::as_str)
}

/// Parses a JSON-lines battery. `file` is only for error messages.
///
/// # Errors
///
/// A line that is not JSON or lacks `id`, `text` or a known `lang`.
pub(crate) fn parse_rows(text: &str, file: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let at = |what: &str| format!("{file}:{}: {what}", n + 1);
        let value: Json = serde_json::from_str(line).map_err(|e| at(&e.to_string()))?;
        let get = |name: &str| field(&value, name).map(str::to_owned);
        rows.push(Row {
            id: get("id").ok_or_else(|| at("no `id`"))?,
            text: get("text").ok_or_else(|| at("no `text`"))?,
            lang: get("lang")
                .ok_or_else(|| at("no `lang`"))?
                .parse::<Lang>()
                .map_err(|e| at(&e.to_string()))?,
            intent: get("intent"),
            kind: get("kind"),
            status: get("status"),
            in_spec: value
                .get("tags")
                .and_then(Json::as_array)
                .is_some_and(|tags| tags.iter().any(|t| t.as_str() == Some("in-spec"))),
        });
    }
    Ok(rows)
}

/// Scores intent rows: correct when the outcome's intent is the label,
/// whatever the status (a recognised intent that cannot be fulfilled keeps
/// its intent, README).
pub(crate) fn score_intents(rows: &[Row], mut run: impl FnMut(&Row) -> Observed) -> IntentScore {
    let mut score = IntentScore::default();
    for row in rows {
        let label = row.intent.clone().unwrap_or_default();
        let observed = run(row).intent;
        let hit = observed.as_deref() == Some(label.as_str());
        score.total.rows += 1;
        if !row.in_spec {
            let fresh = score.held_out.entry(label.clone()).or_default();
            fresh.rows += 1;
            fresh.hits += usize::from(hit);
        }
        let entry = score.per_intent.entry(label).or_default();
        entry.rows += 1;
        if hit {
            score.total.hits += 1;
            entry.hits += 1;
        } else if observed.is_some() {
            score.confused += 1;
        } else {
            score.missed += 1;
        }
    }
    score
}

/// Scores reject rows: a false accept is `answered` with an intent; the
/// status is also compared with the file's.
pub(crate) fn score_reject(rows: &[Row], mut run: impl FnMut(&Row) -> Observed) -> RejectScore {
    let mut score = RejectScore::default();
    for row in rows {
        let observed = run(row);
        let accepted = observed.status == Status::Answered && observed.intent.is_some();
        score.rows += 1;
        let kind = score
            .by_kind
            .entry(row.kind.clone().unwrap_or_else(|| "-".to_owned()))
            .or_default();
        kind.rows += 1;
        if accepted {
            score.false_accepts += 1;
            kind.hits += 1;
        }
        if row.status.as_deref() == Some(observed.status.as_str()) {
            score.status_correct += 1;
        }
    }
    score
}

/// The fixed instant of every row: 2027-03-14 18:42 in UTC+1, as the golden
/// files', so that a run does not depend on the clock.
fn fixed_now() -> Timestamp {
    Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600)
}

/// Runs the conversation battery's text through an engine. Conversations
/// with a `given` (knowledge in the store) are skipped, as in
/// `tests/conversations.rs`.
///
/// # Errors
///
/// A line that is not a conversation.
pub(crate) fn score_conversations(
    engine: &Engine,
    text: &str,
    lang: Lang,
) -> Result<ConversationScore, String> {
    let mut score = ConversationScore::default();
    for (n, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let conversation: Json = serde_json::from_str(line)
            .map_err(|e| format!("conversations.{lang}.jsonl:{}: {e}", n + 1))?;
        if conversation.get("given").is_some() {
            continue;
        }
        score.conversations += 1;
        let id = ConvId::new(format!("eval-{lang}-c{n}"));
        let turns = conversation
            .get("turns")
            .and_then(Json::as_array)
            .ok_or_else(|| format!("conversations.{lang}.jsonl:{}: no `turns`", n + 1))?;
        for t in turns {
            let user = field(t, "user").unwrap_or_default();
            let expected = t.get("expect").and_then(|e| field(e, "status"));
            let outcome =
                engine.respond(Turn::new(user, id.clone(), fixed_now()).with_lang(Some(lang)));
            score.turns += 1;
            if outcome.intent.is_none() && outcome.reason == Some(Reason::CATCH_ALL) {
                score.catch_all += 1;
            }
            if outcome.status == Status::Unhandled && expected != Some("unhandled") {
                score.unexpected_unhandled += 1;
            }
            if expected == Some(outcome.status.as_str()) {
                score.status_correct += 1;
            }
        }
        engine.end(&id);
    }
    Ok(score)
}

/// One row as a fresh single-turn conversation with the row's language as
/// the hint.
fn observe(engine: &Engine, row: &Row) -> Observed {
    let conversation = ConvId::new(format!("eval-{}", row.id));
    let turn =
        Turn::new(row.text.clone(), conversation.clone(), fixed_now()).with_lang(Some(row.lang));
    let outcome = engine.respond(turn);
    engine.end(&conversation);
    Observed {
        status: outcome.status,
        intent: outcome.intent.map(|i| i.as_str().to_owned()),
    }
}

fn read(dir: &Path, name: &str, lang: Lang) -> Result<Option<String>, String> {
    let path = dir.join(format!("{name}.{lang}.jsonl"));
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && name == "conversations" => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn scores(
    engine: &Engine,
    intents: &[Row],
    reject: &[Row],
    conversations: Option<&str>,
    lang: Lang,
) -> Result<Scores, String> {
    Ok(Scores {
        intents: score_intents(intents, |r| observe(engine, r)),
        reject: score_reject(reject, |r| observe(engine, r)),
        conversations: conversations
            .map(|text| score_conversations(engine, text, lang))
            .transpose()?,
    })
}

/// Runs the batteries of `dir` for both languages through both engines:
/// `classified` (rules and classifier) and `rules` (rules only).
///
/// # Errors
///
/// A battery that cannot be read or parsed.
pub(crate) fn run(classified: &Engine, rules: &Engine, dir: &Path) -> Result<Report, String> {
    let languages = LANGS
        .iter()
        .map(|&lang| {
            let parse = |name: &str| -> Result<Vec<Row>, String> {
                let text = read(dir, name, lang)?.unwrap_or_default();
                parse_rows(&text, &format!("{name}.{lang}.jsonl"))
            };
            let (intents, reject) = (parse("intents")?, parse("reject")?);
            let conversations = read(dir, "conversations", lang)?;
            Ok(LangReport {
                lang,
                rules: scores(rules, &intents, &reject, conversations.as_deref(), lang)?,
                classifier: scores(
                    classified,
                    &intents,
                    &reject,
                    conversations.as_deref(),
                    lang,
                )?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let model = match classified.classifiers().intent_model() {
        Ok(model) => format!(
            "intent model with {} classes and {} features, threshold {}, margin {}",
            model.classes().len(),
            model.vocabulary_len(),
            model.threshold(),
            model.margin()
        ),
        Err(why) => format!("no classifier ({why})"),
    };
    Ok(Report {
        skills: classified.skills().iter().map(|m| m.id.clone()).collect(),
        model,
        languages,
    })
}

#[allow(clippy::cast_precision_loss, reason = "row counts are far below 2^52")]
fn percent(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        100.0 * part as f64 / whole as f64
    }
}

/// Intents worst first: lowest accuracy, then most rows, then name.
#[must_use]
pub(crate) fn worst_first(score: &IntentScore) -> Vec<(&str, Count)> {
    let mut list: Vec<(&str, Count)> = score
        .per_intent
        .iter()
        .map(|(intent, count)| (intent.as_str(), *count))
        .collect();
    list.sort_by(|a, b| {
        // Compare a.hits/a.rows with b.hits/b.rows without division.
        (a.1.hits * b.1.rows)
            .cmp(&(b.1.hits * a.1.rows))
            .then(b.1.rows.cmp(&a.1.rows))
            .then(a.0.cmp(b.0))
    });
    list
}

fn ratio(c: Count) -> String {
    format!("{:>4} {:>5.1}%", c.hits, percent(c.hits, c.rows))
}

/// The conversation lines of the text report: rules only, then with the
/// classifier.
fn render_conversations(out: &mut String, l: Lang, ca: ConversationScore, cb: ConversationScore) {
    let count = |hits| Count {
        rows: ca.turns,
        hits,
    };
    let _ = writeln!(
        out,
        "[{l}] conversations ({} turns): catch-all    {} {}",
        ca.turns,
        ratio(count(ca.catch_all)),
        ratio(count(cb.catch_all))
    );
    let _ = writeln!(
        out,
        "[{l}]   unhandled, not expected          {} {}",
        ratio(count(ca.unexpected_unhandled)),
        ratio(count(cb.unexpected_unhandled))
    );
    let _ = writeln!(
        out,
        "[{l}]   status as expected               {} {}",
        ratio(count(ca.status_correct)),
        ratio(count(cb.status_correct))
    );
}

/// The plain-text report.
#[must_use]
pub(crate) fn render_text(report: &Report) -> String {
    let mut out = format!(
        "talos eval: rules only, then rules + classifier ({}); a report, not a gate\n",
        report.model
    );
    for r in &report.languages {
        let (a, b) = (&r.rules, &r.classifier);
        let l = r.lang;
        let _ = writeln!(
            out,
            "\n[{l}] {:<34} {:>11} {:>11}",
            "", "rules", "+classifier"
        );
        let _ = writeln!(
            out,
            "[{l}] intents, all ({} rows)            {} {}",
            a.intents.total.rows,
            ratio(a.intents.total),
            ratio(b.intents.total)
        );
        let (ba, bb) = (
            a.intents.built(&report.skills),
            b.intents.built(&report.skills),
        );
        let _ = writeln!(
            out,
            "[{l}] intents, built skills ({} rows)   {} {}",
            ba.rows,
            ratio(ba),
            ratio(bb)
        );
        let (ha, hb) = (
            a.intents.built_held_out(&report.skills),
            b.intents.built_held_out(&report.skills),
        );
        let _ = writeln!(
            out,
            "[{l}]   not in the spec ({} rows)        {} {}",
            ha.rows,
            ratio(ha),
            ratio(hb)
        );
        let _ = writeln!(
            out,
            "[{l}]   wrong intent / no intent           {:>4} / {:<4}  {:>4} / {:<4}",
            a.intents.confused, a.intents.missed, b.intents.confused, b.intents.missed
        );
        let fa = |s: &Scores| Count {
            rows: s.reject.rows,
            hits: s.reject.false_accepts,
        };
        let _ = writeln!(
            out,
            "[{l}] reject ({} rows): false accepts    {} {}",
            a.reject.rows,
            ratio(fa(a)),
            ratio(fa(b))
        );
        let st = |s: &Scores| Count {
            rows: s.reject.rows,
            hits: s.reject.status_correct,
        };
        let _ = writeln!(
            out,
            "[{l}] reject: status as expected         {} {}",
            ratio(st(a)),
            ratio(st(b))
        );
        for (kind, c) in &b.reject.by_kind {
            let before = a.reject.by_kind.get(kind).map_or(0, |c| c.hits);
            let _ = writeln!(
                out,
                "      {kind:<16} {:>4} rows, false accepts {:>4} {:>11}",
                c.rows, before, c.hits
            );
        }
        if let (Some(ca), Some(cb)) = (a.conversations, b.conversations) {
            render_conversations(&mut out, l, ca, cb);
        }
        let _ = writeln!(
            out,
            "[{l}] per intent, worst first (with the classifier):\n      {:<28} {:>5} {:>11} {:>11}",
            "intent", "rows", "rules", "+classifier"
        );
        for (intent, c) in worst_first(&b.intents) {
            let before = a
                .intents
                .per_intent
                .get(intent)
                .copied()
                .unwrap_or_default();
            let _ = writeln!(
                out,
                "      {intent:<28} {:>5} {} {}",
                c.rows,
                ratio(before),
                ratio(c)
            );
        }
    }
    out
}

fn scores_json(s: &Scores, skills: &BTreeSet<String>) -> Json {
    let per_intent: Vec<Json> = worst_first(&s.intents)
        .into_iter()
        .map(|(intent, c)| json!({ "intent": intent, "rows": c.rows, "correct": c.hits }))
        .collect();
    let by_kind: serde_json::Map<String, Json> = s
        .reject
        .by_kind
        .iter()
        .map(|(k, c)| {
            (
                k.clone(),
                json!({ "rows": c.rows, "false_accepts": c.hits }),
            )
        })
        .collect();
    let built = s.intents.built(skills);
    let held_out = s.intents.built_held_out(skills);
    let conversations = s.conversations.map_or(Json::Null, |c| {
        json!({
            "conversations": c.conversations,
            "turns": c.turns,
            "catch_all": c.catch_all,
            "catch_all_rate": percent(c.catch_all, c.turns) / 100.0,
            "unexpected_unhandled": c.unexpected_unhandled,
            "status_correct": c.status_correct,
        })
    });
    json!({
        "intents": {
            "rows": s.intents.total.rows,
            "correct": s.intents.total.hits,
            "accuracy": percent(s.intents.total.hits, s.intents.total.rows) / 100.0,
            "confused": s.intents.confused,
            "missed": s.intents.missed,
            "built_rows": built.rows,
            "built_correct": built.hits,
            "built_accuracy": percent(built.hits, built.rows) / 100.0,
            "built_held_out_rows": held_out.rows,
            "built_held_out_correct": held_out.hits,
            "built_held_out_accuracy": percent(held_out.hits, held_out.rows) / 100.0,
            "per_intent": per_intent,
        },
        "reject": {
            "rows": s.reject.rows,
            "false_accepts": s.reject.false_accepts,
            "false_accept_rate": percent(s.reject.false_accepts, s.reject.rows) / 100.0,
            "status_correct": s.reject.status_correct,
            "by_kind": by_kind,
        },
        "conversations": conversations,
    })
}

/// The JSON report: per language, the figures with the classifier at the
/// top level and the rules-only ones under `rules_only`.
#[must_use]
pub(crate) fn render_json(report: &Report) -> Json {
    let languages: Vec<Json> = report
        .languages
        .iter()
        .map(|r| {
            let mut entry = scores_json(&r.classifier, &report.skills);
            if let Json::Object(map) = &mut entry {
                map.insert("lang".to_owned(), json!(r.lang.code()));
                map.insert(
                    "rules_only".to_owned(),
                    scores_json(&r.rules, &report.skills),
                );
            }
            entry
        })
        .collect();
    json!({
        "classifier": !report.model.starts_with("no classifier"),
        "model": report.model,
        "skills": report.skills,
        "languages": languages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn intent_row(id: &str, text: &str, intent: &str) -> Row {
        Row {
            id: id.to_owned(),
            text: text.to_owned(),
            lang: Lang::En,
            intent: Some(intent.to_owned()),
            kind: None,
            status: None,
            in_spec: false,
        }
    }

    fn reject_row(id: &str, kind: &str, status: &str) -> Row {
        Row {
            id: id.to_owned(),
            text: id.to_owned(),
            lang: Lang::En,
            intent: None,
            kind: Some(kind.to_owned()),
            status: Some(status.to_owned()),
            in_spec: false,
        }
    }

    fn seen(status: Status, intent: Option<&str>) -> Observed {
        Observed {
            status,
            intent: intent.map(str::to_owned),
        }
    }

    #[test]
    fn parses_rows_and_reports_bad_lines() {
        let text = r#"{"id":"en-1","text":"hi","lang":"en","intent":"smalltalk.greet"}

{"id":"en-r1","text":"x","lang":"en","intent":null,"kind":"noise","status":"unhandled"}
"#;
        let rows = parse_rows(text, "f").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].intent.as_deref(), Some("smalltalk.greet"));
        assert_eq!(rows[1].intent, None);
        assert_eq!(rows[1].status.as_deref(), Some("unhandled"));
        let error = parse_rows("{\"id\":\"a\"}", "f").unwrap_err();
        assert!(error.starts_with("f:1:"), "{error}");
        assert!(parse_rows("nope", "f").is_err());
        assert!(parse_rows(r#"{"id":"a","text":"b","lang":"fr"}"#, "f").is_err());
    }

    #[test]
    fn intent_accuracy_overall_per_intent_and_for_built_skills() {
        let rows = [
            intent_row("1", "a", "x.one"),
            intent_row("2", "b", "x.one"),
            intent_row("3", "c", "y.two"),
            intent_row("4", "d", "y.two"),
            intent_row("5", "e", "z.three"),
        ];
        let score = score_intents(&rows, |row| match row.text.as_str() {
            // Right intent, but unhandled: still correct.
            "a" => seen(Status::Unhandled, Some("x.one")),
            // Wrong intent for "b", right for "c".
            "b" => seen(Status::Answered, Some("z.three")),
            "c" => seen(Status::Answered, Some("y.two")),
            // Nothing recognised.
            _ => seen(Status::Unhandled, None),
        });
        assert_eq!(score.total, Count { rows: 5, hits: 2 });
        assert_eq!(score.confused, 1);
        assert_eq!(score.missed, 2);
        assert_eq!(score.per_intent["x.one"], Count { rows: 2, hits: 1 });
        assert_eq!(score.per_intent["y.two"], Count { rows: 2, hits: 1 });
        assert_eq!(score.per_intent["z.three"], Count { rows: 1, hits: 0 });
        let order: Vec<&str> = worst_first(&score).iter().map(|(i, _)| *i).collect();
        assert_eq!(order, ["z.three", "x.one", "y.two"]);
        let built: BTreeSet<String> = ["x".to_owned(), "z".to_owned()].into();
        assert_eq!(score.built(&built), Count { rows: 3, hits: 1 });
    }

    #[test]
    fn false_accept_needs_answered_with_an_intent() {
        let rows = [
            reject_row("a", "near_miss", "deflected"),
            reject_row("b", "near_miss", "unhandled"),
            reject_row("c", "noise", "answered"),
            reject_row("d", "noise", "unhandled"),
        ];
        let score = score_reject(&rows, |row| match row.id.as_str() {
            "a" => seen(Status::Answered, Some("clock.now")),
            "b" => seen(Status::Unhandled, Some("clock.now")),
            "c" => seen(Status::Answered, None),
            _ => seen(Status::Deflected, None),
        });
        assert_eq!(score.rows, 4);
        assert_eq!(score.false_accepts, 1);
        // "b" and "c" got the status the file expects.
        assert_eq!(score.status_correct, 2);
        assert_eq!(score.by_kind["near_miss"], Count { rows: 2, hits: 1 });
        assert_eq!(score.by_kind["noise"], Count { rows: 2, hits: 0 });
    }

    #[test]
    fn conversations_count_catch_alls_and_unexpected_unhandled() {
        let engine = talos_skills::rules_only_engine(talos_core::Config::default()).unwrap();
        let text = r#"{"id":"c1","lang":"es","turns":[{"user":"hola","expect":{"status":"answered"}},{"user":"zzkx qqw","expect":{"status":"answered"}}]}
{"id":"c2","lang":"es","given":"x","turns":[{"user":"hola","expect":{"status":"answered"}}]}"#;
        let score = score_conversations(&engine, text, Lang::Es).unwrap();
        assert_eq!(score.conversations, 1);
        assert_eq!(score.turns, 2);
        assert_eq!(score.status_correct, 1);
        assert_eq!(
            score.unexpected_unhandled,
            score.turns - score.status_correct
        );
        assert!(score_conversations(&engine, "nope", Lang::Es).is_err());
    }

    #[test]
    fn reports_render_both_ways() {
        let rows = [intent_row("1", "a", "x.one")];
        let scores = |hit: bool| Scores {
            intents: score_intents(&rows, |_| seen(Status::Answered, hit.then_some("x.one"))),
            reject: score_reject(&[], |_| seen(Status::Unhandled, None)),
            conversations: None,
        };
        let report = Report {
            skills: ["x".to_owned()].into(),
            model: "test model".to_owned(),
            languages: vec![LangReport {
                lang: Lang::En,
                rules: scores(false),
                classifier: scores(true),
            }],
        };
        let text = render_text(&report);
        assert!(text.contains("intents, all (1 rows)"), "{text}");
        assert!(text.contains("100.0%"), "{text}");
        assert!(text.contains("x.one"));
        let json = render_json(&report);
        assert_eq!(json["classifier"], true);
        assert_eq!(json["languages"][0]["intents"]["rows"], 1);
        assert_eq!(json["languages"][0]["intents"]["built_correct"], 1);
        assert_eq!(json["languages"][0]["rules_only"]["intents"]["correct"], 0);
        assert_eq!(json["languages"][0]["reject"]["false_accept_rate"], 0.0);
    }
}
