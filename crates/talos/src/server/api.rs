//! The endpoints of docs/API.md section 2: routing, authentication, the
//! OpenAI-shaped requests and responses, and the `talos` extension object.

use std::sync::atomic::Ordering;
use std::time::Instant;

use serde_json::{Value as Json, json};
use talos_core::{Lang, Outcome, Place, Turn};

use super::Shared;
use super::http::{Request, Response};
use super::identity::{Fingerprint, Identity, Speaker};
use crate::wire::talos_object;

/// The one model the server lists.
const MODEL: &str = "talos";

/// An OpenAI-shaped error.
#[derive(Debug)]
pub(crate) struct ApiError {
    status: u16,
    kind: &'static str,
    code: Option<&'static str>,
    param: Option<String>,
    message: String,
}

impl ApiError {
    pub(crate) fn new(status: u16, message: impl Into<String>) -> Self {
        let (kind, code) = match status {
            401 => ("invalid_request_error", Some("invalid_api_key")),
            404 => ("invalid_request_error", Some("not_found")),
            405 => ("invalid_request_error", Some("method_not_allowed")),
            408 => ("invalid_request_error", Some("timeout")),
            413 => ("invalid_request_error", Some("request_too_large")),
            503 => ("server_error", Some("overloaded")),
            s if s >= 500 => ("server_error", None),
            _ => ("invalid_request_error", None),
        };
        Self {
            status,
            kind,
            code,
            param: None,
            message: message.into(),
        }
    }

    fn invalid(param: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            param: Some(param.into()),
            ..Self::new(400, message)
        }
    }

    pub(crate) fn response(&self) -> Response {
        let body = json!({
            "error": {
                "message": self.message,
                "type": self.kind,
                "param": self.param,
                "code": self.code,
            }
        });
        let mut response = Response::json(self.status, to_bytes(&body));
        if self.status == 401 {
            response = response.with_header("WWW-Authenticate", "Bearer");
        }
        response
    }
}

fn to_bytes(value: &Json) -> Vec<u8> {
    // Serialising a `Value` cannot fail: its keys are strings.
    serde_json::to_vec(value).unwrap_or_default()
}

/// Answers one request.
pub(crate) fn handle(shared: &Shared, request: &Request) -> Response {
    if let Err(error) = authorise(shared, request) {
        return error.response();
    }
    let path = request.path.as_str();
    let method = request.method.as_str();
    let result: Answer = match (path, method) {
        ("/health", "GET") => Ok(health(shared)),
        ("/v1/models", "GET") => Ok(models()),
        (p, "GET") if p.starts_with("/v1/models/") => model(&p["/v1/models/".len()..]),
        ("/v1/chat/completions", "POST") => chat_completions(shared, request),
        ("/health" | "/v1/models", _) => Err(not_allowed("GET")),
        (p, _) if p.starts_with("/v1/models/") => Err(not_allowed("GET")),
        ("/v1/chat/completions", _) => Err(not_allowed("POST")),
        _ => Err(ApiError::new(404, format!("no route for {method} {path}")).into()),
    };
    match result {
        Ok(response) => response,
        Err((error, allow)) => {
            let response = error.response();
            match allow {
                Some(allow) => response.with_header("Allow", allow),
                None => response,
            }
        }
    }
}

type Answer = Result<Response, (ApiError, Option<&'static str>)>;

fn not_allowed(allow: &'static str) -> (ApiError, Option<&'static str>) {
    (
        ApiError::new(405, format!("this route only accepts {allow}")),
        Some(allow),
    )
}

impl From<ApiError> for (ApiError, Option<&'static str>) {
    fn from(error: ApiError) -> Self {
        (error, None)
    }
}

/// With a token configured, every request must carry it as a bearer token.
fn authorise(shared: &Shared, request: &Request) -> Result<(), ApiError> {
    let Some(token) = &shared.token else {
        return Ok(());
    };
    let given = request.header("authorization").and_then(|value| {
        let (scheme, credentials) = value.split_once(' ')?;
        scheme
            .eq_ignore_ascii_case("bearer")
            .then(|| credentials.trim())
    });
    match given {
        Some(given) if same(given.as_bytes(), token.as_bytes()) => Ok(()),
        Some(_) => Err(ApiError::new(401, "invalid bearer token")),
        None => Err(ApiError::new(
            401,
            "this server requires an `Authorization: Bearer` token",
        )),
    }
}

/// Compares two secrets in time that depends only on their lengths.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `GET /health`.
fn health(shared: &Shared) -> Response {
    let skills: Vec<&str> = shared
        .engine
        .skills()
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    let body = json!({
        "status": "ok",
        "version": crate::VERSION,
        "skills": skills,
        "packs": [],
        "failures": [],
    });
    Response::json(200, to_bytes(&body))
}

fn model_object() -> Json {
    json!({ "id": MODEL, "object": "model", "created": 0, "owned_by": "talos" })
}

/// `GET /v1/models`.
fn models() -> Response {
    let body = json!({ "object": "list", "data": [model_object()] });
    Response::json(200, to_bytes(&body))
}

/// `GET /v1/models/{id}`.
fn model(id: &str) -> Answer {
    if id == MODEL {
        Ok(Response::json(200, to_bytes(&model_object())))
    } else {
        Err(ApiError::new(
            404,
            format!("the model `{id}` does not exist; the only model is `talos`"),
        )
        .into())
    }
}

/// What the server reads from a chat-completions request.
#[derive(Debug)]
struct ChatRequest {
    /// The last user message.
    text: String,
    identity: Identity,
    /// The fingerprint of the transcript up to and including the last user
    /// message: the reply is appended to it for the next request.
    through_text: Fingerprint,
    stream: bool,
    include_usage: bool,
    lang: Option<Lang>,
    place: Option<Place>,
    trace: bool,
}

/// The text of a message's `content`: a string, an array of parts whose
/// text parts are joined by newlines (other parts are ignored), or null.
fn content_text(content: Option<&Json>, param: &str) -> Result<String, ApiError> {
    match content {
        None | Some(Json::Null) => Ok(String::new()),
        Some(Json::String(text)) => Ok(text.clone()),
        Some(Json::Array(parts)) => {
            let mut texts = Vec::new();
            for (i, part) in parts.iter().enumerate() {
                match part {
                    Json::String(text) => texts.push(text.as_str()),
                    Json::Object(part) => {
                        if part.get("type").and_then(Json::as_str) == Some("text") {
                            let Some(text) = part.get("text").and_then(Json::as_str) else {
                                return Err(ApiError::invalid(
                                    format!("{param}[{i}].text"),
                                    "a text part needs a string `text`",
                                ));
                            };
                            texts.push(text);
                        }
                    }
                    _ => {
                        return Err(ApiError::invalid(
                            format!("{param}[{i}]"),
                            "a content part must be an object",
                        ));
                    }
                }
            }
            Ok(texts.join("\n"))
        }
        Some(_) => Err(ApiError::invalid(
            param,
            "`content` must be a string, an array of parts or null",
        )),
    }
}

/// A string field that may be absent or null; any other type is an error.
fn optional_str<'a>(value: Option<&'a Json>, param: &str) -> Result<Option<&'a str>, ApiError> {
    match value {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(ApiError::invalid(
            param,
            format!("`{param}` must be a string"),
        )),
    }
}

/// A boolean field that may be absent or null. `metadata` values are strings
/// in the `OpenAI` API, so `"true"` and `"false"` are accepted too.
fn optional_bool(value: Option<&Json>, param: &str) -> Result<bool, ApiError> {
    match value {
        None | Some(Json::Null) => Ok(false),
        Some(Json::Bool(b)) => Ok(*b),
        Some(Json::String(s)) if s == "true" => Ok(true),
        Some(Json::String(s)) if s == "false" => Ok(false),
        Some(_) => Err(ApiError::invalid(
            param,
            format!("`{param}` must be a boolean"),
        )),
    }
}

/// Reads the fields Talos uses; everything else is ignored.
fn parse_chat(request: &Request, default_lang: Option<Lang>) -> Result<ChatRequest, ApiError> {
    let body: Json = serde_json::from_slice(&request.body)
        .map_err(|e| ApiError::new(400, format!("the body is not valid JSON: {e}")))?;
    let Json::Object(body) = body else {
        return Err(ApiError::new(400, "the body must be a JSON object"));
    };
    let Some(Json::Array(messages)) = body.get("messages") else {
        return Err(ApiError::invalid(
            "messages",
            "`messages` must be an array of messages",
        ));
    };
    let mut roles = Vec::with_capacity(messages.len());
    for (i, message) in messages.iter().enumerate() {
        let role = message.get("role").and_then(Json::as_str).ok_or_else(|| {
            ApiError::invalid(
                format!("messages[{i}].role"),
                "every message needs a string `role`",
            )
        })?;
        roles.push(role);
    }
    let Some(last) = roles.iter().rposition(|r| *r == "user") else {
        return Err(ApiError::invalid(
            "messages",
            "`messages` must contain a user message",
        ));
    };
    let mut prefix = Fingerprint::EMPTY;
    for (i, (message, role)) in messages.iter().zip(&roles).enumerate().take(last) {
        let speaker = match *role {
            "user" => Speaker::User,
            "assistant" => Speaker::Assistant,
            _ => continue,
        };
        let text = content_text(message.get("content"), &format!("messages[{i}].content"))?;
        prefix = prefix.then(speaker, &text);
    }
    let text = content_text(
        messages[last].get("content"),
        &format!("messages[{last}].content"),
    )?;

    let metadata = match body.get("metadata") {
        None | Some(Json::Null) => None,
        Some(Json::Object(map)) => Some(map),
        Some(_) => {
            return Err(ApiError::invalid(
                "metadata",
                "`metadata` must be an object",
            ));
        }
    };
    let meta = |key: &str| metadata.and_then(|m| m.get(key));
    let lang = match optional_str(meta("language"), "metadata.language")? {
        None => default_lang,
        Some(code) => Some(code.parse::<Lang>().map_err(|_| {
            ApiError::invalid(
                "metadata.language",
                "`metadata.language` must be `es` or `en`",
            )
        })?),
    };
    let place = optional_str(meta("place"), "metadata.place")?
        .filter(|p| !p.trim().is_empty())
        .map(|p| Place(p.trim().to_owned()));
    let trace = optional_bool(meta("trace"), "metadata.trace")?;
    let key = optional_str(meta("conversation"), "metadata.conversation")?
        .or_else(|| request.header("x-talos-conversation"))
        .filter(|k| !k.is_empty());
    let user = optional_str(body.get("user"), "user")?.filter(|u| !u.is_empty());
    let identity = match (key, user) {
        (Some(key), _) => Identity::Key(key.to_owned()),
        (None, Some(user)) => Identity::User(user.to_owned()),
        (None, None) => Identity::Transcript(prefix),
    };
    let stream = optional_bool(body.get("stream"), "stream")?;
    let include_usage = match body.get("stream_options") {
        Some(Json::Object(options)) => {
            optional_bool(options.get("include_usage"), "stream_options.include_usage")?
        }
        _ => false,
    };
    Ok(ChatRequest {
        through_text: prefix.then(Speaker::User, &text),
        text,
        identity,
        stream,
        include_usage,
        lang,
        place,
        trace,
    })
}

/// `POST /v1/chat/completions`.
fn chat_completions(shared: &Shared, request: &Request) -> Answer {
    let chat = parse_chat(request, shared.default_lang)?;
    let now = (shared.clock)();
    let conversation = shared.transcripts.resolve(&chat.identity, now);
    let mut turn = Turn::new(chat.text, conversation.clone(), now)
        .with_lang(chat.lang)
        .with_place(chat.place);
    if chat.trace {
        turn = turn.with_trace();
    }
    let started = Instant::now();
    let outcome = shared.engine.respond(turn);
    let elapsed = if shared.stopwatch {
        started.elapsed().as_micros()
    } else {
        0
    };
    if matches!(chat.identity, Identity::Transcript(_)) {
        let next = chat.through_text.then(Speaker::Assistant, &outcome.reply);
        shared.transcripts.remember(next, &conversation, now);
    }
    let number = shared.completions.fetch_add(1, Ordering::Relaxed) + 1;
    let completion = Completion {
        id: format!("talos-{number:016x}"),
        created: now.unix_seconds,
        outcome: &outcome,
        talos: talos_object(&outcome, elapsed),
    };
    let response = if chat.stream {
        let mut response = Response::json(200, completion.events(chat.include_usage));
        response.content_type = "text/event-stream";
        response.with_header("Cache-Control", "no-cache")
    } else {
        Response::json(200, to_bytes(&completion.whole()))
    };
    Ok(response.with_header("X-Talos-Status", outcome.status.as_str()))
}

/// One answered turn, in either of its two shapes.
struct Completion<'a> {
    id: String,
    created: i64,
    outcome: &'a Outcome,
    talos: Json,
}

impl Completion<'_> {
    /// The `chat.completion` object.
    fn whole(&self) -> Json {
        json!({
            "id": self.id,
            "object": "chat.completion",
            "created": self.created,
            "model": MODEL,
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": self.outcome.reply },
                "logprobs": null,
                "finish_reason": "stop",
            }],
            "usage": usage(),
            "talos": self.talos,
        })
    }

    /// A `chat.completion.chunk` with one choice.
    fn chunk(&self, delta: &Json, finish: Option<&str>) -> Json {
        json!({
            "id": self.id,
            "object": "chat.completion.chunk",
            "created": self.created,
            "model": MODEL,
            "choices": [{
                "index": 0,
                "delta": delta,
                "logprobs": null,
                "finish_reason": finish,
            }],
        })
    }

    /// The server-sent events of a streamed reply: the role, the whole
    /// reply in one chunk (it exists before the first byte is sent), the
    /// finish with the `talos` object, the usage when asked for, `[DONE]`.
    fn events(&self, include_usage: bool) -> Vec<u8> {
        let mut chunks = vec![
            self.chunk(&json!({ "role": "assistant", "content": "" }), None),
            self.chunk(&json!({ "content": self.outcome.reply }), None),
            self.chunk(&json!({}), Some("stop")),
        ];
        if let Some(Json::Object(last)) = chunks.last_mut() {
            last.insert("talos".to_owned(), self.talos.clone());
        }
        if include_usage {
            for chunk in &mut chunks {
                if let Json::Object(chunk) = chunk {
                    chunk.insert("usage".to_owned(), Json::Null);
                }
            }
            chunks.push(json!({
                "id": self.id,
                "object": "chat.completion.chunk",
                "created": self.created,
                "model": MODEL,
                "choices": [],
                "usage": usage(),
            }));
        }
        let mut out = Vec::new();
        for chunk in &chunks {
            out.extend_from_slice(b"data: ");
            out.extend_from_slice(&to_bytes(chunk));
            out.extend_from_slice(b"\n\n");
        }
        out.extend_from_slice(b"data: [DONE]\n\n");
        out
    }
}

/// Token counts mean nothing for Talos and are reported as zero (ADR 0010).
fn usage() -> Json {
    json!({ "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 })
}
