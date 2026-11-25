//! The outcome on the wire: the `talos` object of docs/API.md section 2.1,
//! shared by `talos ask --json` and the server.

use serde_json::{Map, Value as Json, json};
use talos_core::{Outcome, Value};

/// A slot value on the wire: a list as an array of strings, anything else
/// as its canonical text (numbers with a decimal point, dates in ISO 8601),
/// so that no number loses exactness.
#[must_use]
pub fn slot_json(value: &Value) -> Json {
    match value {
        Value::List { items, .. } => json!(items),
        other => json!(other.to_string()),
    }
}

/// The `talos` extension object: everything about the outcome but the reply
/// text. With a trace, it also carries `trace` and `internal_slots`.
#[must_use]
pub fn talos_object(outcome: &Outcome, elapsed_us: u128) -> Json {
    let slots: Map<String, Json> = outcome
        .slots
        .iter()
        .map(|(name, value)| (name.clone(), slot_json(value)))
        .collect();
    let mut object = json!({
        "status": outcome.status.as_str(),
        "intent": outcome.intent.as_ref().map(talos_core::IntentId::as_str),
        "slots": slots,
        "confidence": outcome.confidence,
        "language": outcome.lang.code(),
        "reason": outcome.reason.as_ref().map(talos_core::Reason::as_str),
        "pending": outcome.pending,
        "generated": outcome.generated,
        "elapsed_us": u64::try_from(elapsed_us).unwrap_or(u64::MAX),
    });
    // Present only when the reply rests on a cheaper source (DESIGN.md
    // principle 8; API.md 2.2).
    if let (Some(fallback), Some(map)) = (&outcome.fallback, object.as_object_mut()) {
        map.insert("fallback".to_owned(), json!(fallback.as_str()));
    }
    if let (Some(trace), Some(map)) = (&outcome.trace, object.as_object_mut()) {
        let steps: Vec<Json> = trace
            .steps
            .iter()
            .map(|s| json!({ "stage": s.stage, "note": s.note }))
            .collect();
        let internal: Map<String, Json> = trace
            .internal_slots
            .iter()
            .map(|(name, value)| (name.clone(), slot_json(value)))
            .collect();
        map.insert("trace".to_owned(), json!(steps));
        map.insert("internal_slots".to_owned(), json!(internal));
    }
    object
}

/// The full outcome of `talos ask --json` and `talos explain --json`: the
/// `talos` object plus the reply.
#[must_use]
pub fn outcome_json(outcome: &Outcome, elapsed_us: u128) -> Json {
    let mut object = talos_object(outcome, elapsed_us);
    if let Some(map) = object.as_object_mut() {
        map.insert("reply".to_owned(), json!(outcome.reply));
    }
    object
}

#[cfg(test)]
mod tests {
    use super::*;
    use talos_core::{Config, ConvId, Timestamp, Turn};

    #[test]
    fn the_json_outcome_uses_the_api_field_names() {
        let engine = talos_skills::engine(Config::default()).unwrap();
        let turn = Turn::new("hello", ConvId::new("t"), Timestamp::from_unix(0));
        let json = outcome_json(&engine.respond(turn), 12);
        assert_eq!(json["status"], "answered");
        assert_eq!(json["intent"], "smalltalk.greet");
        assert_eq!(json["language"], "en");
        assert_eq!(json["confidence"], 1.0);
        assert_eq!(json["reason"], Json::Null);
        assert_eq!(json["pending"], false);
        assert_eq!(json["generated"], false);
        assert_eq!(json["elapsed_us"], 12);
        assert_eq!(json["reply"], "Hello!");
        assert!(json["slots"].as_object().unwrap().is_empty());
        assert!(json.get("trace").is_none());
        assert!(json.get("fallback").is_none());
        let mut fell_back = engine.respond(Turn::new(
            "hello",
            ConvId::new("u"),
            Timestamp::from_unix(0),
        ));
        fell_back.fallback = Some(talos_core::Fallback::WORD_PAIRS);
        assert_eq!(outcome_json(&fell_back, 1)["fallback"], "word-pairs");
    }
}
