//! `talos eval`: runs the intent batteries of `evals/` (format in
//! `evals/README.md`) through the engine as it is, rules only, and reports
//! per language the intent accuracy and the false-accept rate of the
//! rejection set. A baseline, not a gate: the command exits 0 whatever the
//! scores.
//!
//! Every row is a fresh single-turn conversation, with the row's language
//! as the caller's hint. The phrases are a test set: never copy them into a
//! manifest or a script (evals/README.md, independence rule).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value as Json, json};
use talos_core::{ConvId, Engine, Lang, Status, Timestamp, Turn};

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
    pub(crate) per_intent: BTreeMap<String, Count>,
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

/// Both batteries of one language.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LangReport {
    pub(crate) lang: Lang,
    pub(crate) intents: IntentScore,
    pub(crate) reject: RejectScore,
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
        let hit = run(row).intent.as_deref() == Some(label.as_str());
        score.total.rows += 1;
        let entry = score.per_intent.entry(label).or_default();
        entry.rows += 1;
        if hit {
            score.total.hits += 1;
            entry.hits += 1;
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

/// Reads `intents.<lang>.jsonl` and `reject.<lang>.jsonl` from `dir`.
///
/// # Errors
///
/// A file that cannot be read or parsed.
pub(crate) fn load(dir: &Path, lang: Lang) -> Result<(Vec<Row>, Vec<Row>), String> {
    let read = |name: &str| -> Result<Vec<Row>, String> {
        let path = dir.join(format!("{name}.{lang}.jsonl"));
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        parse_rows(&text, &path.display().to_string())
    };
    Ok((read("intents")?, read("reject")?))
}

/// The fixed instant of every row: 2027-03-14 18:42 in UTC+1, as the golden
/// files', so that a run does not depend on the clock.
fn fixed_now() -> Timestamp {
    Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600)
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

/// Runs the batteries of `dir` for both languages.
///
/// # Errors
///
/// A battery that cannot be read or parsed.
pub(crate) fn run(engine: &Engine, dir: &Path) -> Result<Vec<LangReport>, String> {
    LANGS
        .iter()
        .map(|&lang| {
            let (intents, reject) = load(dir, lang)?;
            Ok(LangReport {
                lang,
                intents: score_intents(&intents, |r| observe(engine, r)),
                reject: score_reject(&reject, |r| observe(engine, r)),
            })
        })
        .collect()
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

/// The plain-text report.
#[must_use]
pub(crate) fn render_text(reports: &[LangReport]) -> String {
    let mut out = String::from("talos eval: rules only, no classifier; a baseline, not a gate\n");
    for r in reports {
        let t = r.intents.total;
        let _ = writeln!(
            out,
            "\n[{}] intents: {} rows, {} correct, accuracy {:.1}%",
            r.lang,
            t.rows,
            t.hits,
            percent(t.hits, t.rows)
        );
        let j = &r.reject;
        let _ = writeln!(
            out,
            "[{}] reject:  {} rows, {} false accepts ({:.1}%), status as expected {} ({:.1}%)",
            r.lang,
            j.rows,
            j.false_accepts,
            percent(j.false_accepts, j.rows),
            j.status_correct,
            percent(j.status_correct, j.rows)
        );
        for (kind, c) in &j.by_kind {
            let _ = writeln!(
                out,
                "      {kind:<16} {:>4} rows {:>4} false accepts",
                c.rows, c.hits
            );
        }
        let _ = writeln!(
            out,
            "[{}] per intent, worst first:\n      {:<28} {:>5} {:>8} {:>7}",
            r.lang, "intent", "rows", "correct", "acc"
        );
        for (intent, c) in worst_first(&r.intents) {
            let _ = writeln!(
                out,
                "      {intent:<28} {:>5} {:>8} {:>6.1}%",
                c.rows,
                c.hits,
                percent(c.hits, c.rows)
            );
        }
    }
    out
}

/// The JSON report.
#[must_use]
pub(crate) fn render_json(reports: &[LangReport]) -> Json {
    let languages: Vec<Json> = reports
        .iter()
        .map(|r| {
            let per_intent: Vec<Json> = worst_first(&r.intents)
                .into_iter()
                .map(|(intent, c)| json!({ "intent": intent, "rows": c.rows, "correct": c.hits }))
                .collect();
            let by_kind: serde_json::Map<String, Json> = r
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
            json!({
                "lang": r.lang.code(),
                "intents": {
                    "rows": r.intents.total.rows,
                    "correct": r.intents.total.hits,
                    "accuracy": percent(r.intents.total.hits, r.intents.total.rows) / 100.0,
                    "per_intent": per_intent,
                },
                "reject": {
                    "rows": r.reject.rows,
                    "false_accepts": r.reject.false_accepts,
                    "false_accept_rate": percent(r.reject.false_accepts, r.reject.rows) / 100.0,
                    "status_correct": r.reject.status_correct,
                    "by_kind": by_kind,
                },
            })
        })
        .collect();
    json!({ "classifier": false, "languages": languages })
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
    fn intent_accuracy_overall_and_per_intent() {
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
        assert_eq!(score.per_intent["x.one"], Count { rows: 2, hits: 1 });
        assert_eq!(score.per_intent["y.two"], Count { rows: 2, hits: 1 });
        assert_eq!(score.per_intent["z.three"], Count { rows: 1, hits: 0 });
        let order: Vec<&str> = worst_first(&score).iter().map(|(i, _)| *i).collect();
        assert_eq!(order, ["z.three", "x.one", "y.two"]);
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
    fn reports_render_both_ways() {
        let rows = [intent_row("1", "a", "x.one")];
        let report = LangReport {
            lang: Lang::En,
            intents: score_intents(&rows, |_| seen(Status::Answered, Some("x.one"))),
            reject: score_reject(&[], |_| seen(Status::Unhandled, None)),
        };
        let text = render_text(std::slice::from_ref(&report));
        assert!(text.contains("[en] intents: 1 rows, 1 correct, accuracy 100.0%"));
        assert!(text.contains("x.one"));
        let json = render_json(std::slice::from_ref(&report));
        assert_eq!(json["languages"][0]["intents"]["rows"], 1);
        assert_eq!(json["languages"][0]["reject"]["false_accept_rate"], 0.0);
    }
}
