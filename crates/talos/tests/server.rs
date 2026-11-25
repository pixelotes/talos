//! `talos serve` end to end (docs/API.md section 2): servers on ephemeral
//! ports, spoken to with a plain HTTP/1.1 client written here.

#![cfg(feature = "server")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "test harness; a failure is a test failure"
)]

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value as Json, json};
use talos::server::{Server, Settings, StartError};
use talos_core::{Config, Timestamp};

/// The fixed instant of every test server: 2027-03-14 18:42 in Madrid.
fn fixed_now() -> Timestamp {
    Timestamp::from_unix(1_805_046_120).with_utc_offset(3_600)
}

fn settings() -> Settings {
    let mut settings = Settings::new("127.0.0.1:0".parse().unwrap());
    settings.clock = std::sync::Arc::new(fixed_now);
    settings.stopwatch = false;
    settings
}

/// Starts a server on a free port with a fixed clock and no stopwatch.
fn start_with(config: Config, adjust: impl FnOnce(&mut Settings)) -> SocketAddr {
    let mut settings = settings();
    adjust(&mut settings);
    let engine = talos_skills::engine(config).unwrap();
    let server = Server::bind(engine, settings).unwrap();
    let addr = server.local_addr().unwrap();
    std::thread::spawn(move || server.run());
    addr
}

fn start() -> SocketAddr {
    start_with(Config::default(), |_| {})
}

/// One response.
#[derive(Debug)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Json {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&self.body)))
    }

    fn text(&self) -> String {
        String::from_utf8(self.body.clone()).unwrap()
    }

    /// The reply text of a non-streamed completion.
    fn content(&self) -> String {
        self.json()["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .to_owned()
    }
}

/// A keep-alive connection.
struct Conn {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
}

impl Conn {
    fn open(addr: SocketAddr) -> Self {
        let stream = TcpStream::connect(addr).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        Self {
            reader: BufReader::new(stream.try_clone().unwrap()),
            writer: stream,
        }
    }

    fn send_raw(&mut self, raw: &[u8]) {
        self.writer.write_all(raw).unwrap();
    }

    /// Reads one response; `None` if the server closed first.
    fn read(&mut self) -> Option<Reply> {
        let mut line = String::new();
        if self.reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let status: u16 = line.split(' ').nth(1).unwrap().parse().unwrap();
        let mut headers = Vec::new();
        loop {
            line.clear();
            self.reader.read_line(&mut line).unwrap();
            let header = line.trim_end();
            if header.is_empty() {
                break;
            }
            let (name, value) = header.split_once(':').unwrap();
            headers.push((name.to_owned(), value.trim().to_owned()));
        }
        if status < 200 {
            return Some(Reply {
                status,
                headers,
                body: Vec::new(),
            });
        }
        let length: usize = headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("content-length"))
            .map(|(_, v)| v.parse().unwrap())
            .expect("every response has a Content-Length");
        let mut body = vec![0; length];
        self.reader.read_exact(&mut body).unwrap();
        Some(Reply {
            status,
            headers,
            body,
        })
    }

    fn request(&mut self, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> Reply {
        let mut raw = format!("{method} {path} HTTP/1.1\r\nHost: test\r\n");
        for (name, value) in headers {
            let _ = write!(raw, "{name}: {value}\r\n");
        }
        if !body.is_empty() || method == "POST" {
            let _ = write!(raw, "Content-Length: {}\r\n", body.len());
        }
        raw.push_str("\r\n");
        raw.push_str(body);
        self.send_raw(raw.as_bytes());
        self.read().expect("a response")
    }

    fn chat(&mut self, body: &Json) -> Reply {
        self.request(
            "POST",
            "/v1/chat/completions",
            &[("Content-Type", "application/json")],
            &body.to_string(),
        )
    }

    /// Whether the server has closed the connection.
    fn closed(&mut self) -> bool {
        let mut byte = [0];
        matches!(self.reader.read(&mut byte), Ok(0))
    }
}

fn get(addr: SocketAddr, path: &str) -> Reply {
    Conn::open(addr).request("GET", path, &[], "")
}

fn chat(addr: SocketAddr, body: &Json) -> Reply {
    Conn::open(addr).chat(body)
}

fn user(text: &str) -> Json {
    json!({ "model": "talos", "messages": [{ "role": "user", "content": text }] })
}

fn is_english_greeting(text: &str) -> bool {
    ["Hello!", "Hi!"].contains(&text)
}

/// The error object of an error response, checked for the `OpenAI` shape.
fn error_of(reply: &Reply, status: u16) -> Json {
    assert_eq!(reply.status, status, "{}", reply.text());
    assert_eq!(reply.header("content-type"), Some("application/json"));
    let error = reply.json()["error"].clone();
    assert!(error["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert!(error["type"].is_string());
    assert!(error.get("param").is_some() && error.get("code").is_some());
    error
}

#[test]
fn health_and_models() {
    let addr = start();
    let health = get(addr, "/health");
    assert_eq!(health.status, 200);
    let body = health.json();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], talos::VERSION);
    let skills: Vec<&str> = body["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert!(
        skills.contains(&"smalltalk") && skills.contains(&"clock"),
        "{skills:?}"
    );
    assert_eq!(body["packs"], json!([]));
    assert_eq!(body["failures"], json!([]));

    let models = get(addr, "/v1/models");
    assert_eq!(models.status, 200);
    let body = models.json();
    assert_eq!(body["object"], "list");
    assert_eq!(body["data"].as_array().unwrap().len(), 1);
    assert_eq!(body["data"][0]["id"], "talos");
    assert_eq!(body["data"][0]["object"], "model");
    assert_eq!(get(addr, "/v1/models/talos").json()["id"], "talos");
    error_of(&get(addr, "/v1/models/gpt-4o"), 404);
    assert!(health.header("x-talos-status").is_none());
}

#[test]
fn a_completion() {
    let addr = start();
    let reply = chat(addr, &user("hola"));
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("content-type"), Some("application/json"));
    assert_eq!(reply.header("x-talos-status"), Some("answered"));
    let body = reply.json();
    assert!(body["id"].as_str().unwrap().starts_with("talos-"));
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["created"], 1_805_046_120);
    assert_eq!(body["model"], "talos");
    let choice = &body["choices"][0];
    assert_eq!(choice["index"], 0);
    assert_eq!(choice["message"]["role"], "assistant");
    assert_eq!(choice["message"]["content"], "¡Hola!");
    assert_eq!(choice["finish_reason"], "stop");
    assert_eq!(
        body["usage"],
        json!({ "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 })
    );
    let talos = &body["talos"];
    assert_eq!(talos["status"], "answered");
    assert_eq!(talos["intent"], "smalltalk.greet");
    assert_eq!(talos["language"], "es");
    assert_eq!(talos["confidence"], 1.0);
    assert_eq!(talos["slots"], json!({}));
    assert_eq!(talos["reason"], Json::Null);
    assert_eq!(talos["pending"], false);
    assert_eq!(talos["generated"], false);
    assert_eq!(talos["elapsed_us"], 0);
    assert!(talos.get("trace").is_none());
    assert!(talos.get("reply").is_none());
}

#[test]
fn a_turn_talos_cannot_answer_is_not_an_error() {
    let addr = start();
    let reply = chat(addr, &user("escríbeme un poema"));
    assert_eq!(reply.status, 200);
    let body = reply.json();
    assert_eq!(body["talos"]["status"], "unhandled");
    assert_eq!(reply.header("x-talos-status"), Some("unhandled"));
    assert!(body["talos"]["reason"].is_string());
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "No sé escribir textos nuevos."
    );
}

#[test]
fn hints_in_metadata() {
    let addr = start();
    let mut body = user("hola");
    body["metadata"] = json!({ "language": "en", "trace": "true", "place": "Madrid" });
    let reply = chat(addr, &body).json();
    assert_eq!(reply["talos"]["language"], "en");
    assert!(is_english_greeting(
        reply["choices"][0]["message"]["content"].as_str().unwrap()
    ));
    let trace = reply["talos"]["trace"].as_array().unwrap();
    assert!(trace.iter().any(|s| s["stage"] == "language"), "{trace:?}");
    assert!(reply["talos"]["internal_slots"].is_object());
}

/// The data lines of a server-sent event stream, parsed; `[DONE]` last.
fn events(reply: &Reply) -> (Vec<Json>, bool) {
    let text = reply.text();
    assert!(text.ends_with("\n\n"));
    let mut chunks = Vec::new();
    let mut done = false;
    for event in text.split("\n\n").filter(|e| !e.is_empty()) {
        let data = event
            .strip_prefix("data: ")
            .expect("one data line per event");
        assert!(!done, "nothing after [DONE]");
        if data == "[DONE]" {
            done = true;
        } else {
            chunks.push(serde_json::from_str(data).unwrap());
        }
    }
    (chunks, done)
}

#[test]
fn a_streamed_completion() {
    let addr = start();
    let mut body = user("hola");
    body["stream"] = json!(true);
    let reply = chat(addr, &body);
    assert_eq!(reply.status, 200);
    assert_eq!(reply.header("content-type"), Some("text/event-stream"));
    assert_eq!(reply.header("x-talos-status"), Some("answered"));
    let (chunks, done) = events(&reply);
    assert!(done);
    assert_eq!(chunks.len(), 3);
    let id = &chunks[0]["id"];
    let mut content = String::new();
    for chunk in &chunks {
        assert_eq!(chunk["object"], "chat.completion.chunk");
        assert_eq!(&chunk["id"], id);
        assert_eq!(chunk["model"], "talos");
        assert!(chunk.get("usage").is_none());
        let choice = &chunk["choices"][0];
        assert_eq!(choice["index"], 0);
        content.push_str(choice["delta"]["content"].as_str().unwrap_or_default());
    }
    assert_eq!(content, "¡Hola!");
    assert_eq!(chunks[0]["choices"][0]["delta"]["role"], "assistant");
    assert_eq!(chunks[0]["choices"][0]["finish_reason"], Json::Null);
    let last = &chunks[2];
    assert_eq!(last["choices"][0]["finish_reason"], "stop");
    assert_eq!(last["choices"][0]["delta"], json!({}));
    assert_eq!(last["talos"]["status"], "answered");
    assert_eq!(last["talos"]["intent"], "smalltalk.greet");

    // With usage asked for: `usage: null` on every chunk, then one more.
    body["stream_options"] = json!({ "include_usage": true });
    let (chunks, done) = events(&chat(addr, &body));
    assert!(done);
    assert_eq!(chunks.len(), 4);
    assert!(chunks[..3].iter().all(|c| c["usage"].is_null()));
    assert_eq!(chunks[3]["choices"], json!([]));
    assert_eq!(chunks[3]["usage"]["total_tokens"], 0);
}

/// Two turns of one conversation, by whatever `second` adds to the request:
/// "hola" settles Spanish, so a lone English word stays in Spanish only if
/// the state was kept (docs/spec/conversation.md, language stickiness).
fn kept(addr: SocketAddr, first: &Json, second: &Json) -> bool {
    let reply = chat(addr, first);
    assert_eq!(reply.content(), "¡Hola!");
    let reply = chat(addr, second).content();
    if reply.starts_with("¡Hola") {
        true
    } else {
        assert!(is_english_greeting(&reply), "{reply}");
        false
    }
}

#[test]
fn a_fresh_conversation_answers_in_the_language_it_is_spoken_to() {
    let addr = start();
    assert!(is_english_greeting(&chat(addr, &user("hello")).content()));
}

#[test]
fn identity_by_explicit_key() {
    let addr = start();
    let with = |text: &str, key: &str| {
        let mut body = user(text);
        body["metadata"] = json!({ "conversation": key });
        body
    };
    assert!(kept(addr, &with("hola", "k1"), &with("hello", "k1")));
    assert!(!kept(addr, &with("hola", "k2"), &with("hello", "k3")));
    // The header does the same, and the body's key wins over it.
    let header = [("X-Talos-Conversation", "h1")];
    let raw = |text: &str| user(text).to_string();
    let mut conn = Conn::open(addr);
    let first = conn.request("POST", "/v1/chat/completions", &header, &raw("hola"));
    assert_eq!(first.content(), "¡Hola!");
    let second = conn.request("POST", "/v1/chat/completions", &header, &raw("hello"));
    assert!(second.content().starts_with("¡Hola"));
    let body = with("hello", "elsewhere").to_string();
    let third = conn.request("POST", "/v1/chat/completions", &header, &body);
    assert!(is_english_greeting(&third.content()));
}

/// An open question spans two requests of one conversation: `pending` is
/// true while it is open, and the next message answers it
/// (docs/spec/conversation.md 3.2, docs/API.md 2.2).
#[test]
fn an_open_question_across_two_requests() {
    let addr = start();
    let with = |text: &str| {
        let mut body = user(text);
        body["metadata"] = json!({ "conversation": "question" });
        body
    };
    let first = chat(addr, &with("calcula"));
    assert_eq!(first.header("x-talos-status"), Some("answered"));
    assert_eq!(first.content(), "¿Qué quieres calcular?");
    let talos = &first.json()["talos"];
    assert_eq!(talos["pending"], true);
    assert_eq!(talos["status"], "answered");
    assert_eq!(talos["intent"], "calc.arithmetic");
    let second = chat(addr, &with("12 por 12"));
    assert_eq!(second.content(), "12 × 12 = 144.");
    let talos = &second.json()["talos"];
    assert_eq!(talos["pending"], false);
    assert_eq!(talos["intent"], "calc.arithmetic");
    assert_eq!(talos["slots"]["expression"], "12*12");
    // Another conversation has nothing open.
    let mut elsewhere = user("12 por 12");
    elsewhere["metadata"] = json!({ "conversation": "other" });
    assert_eq!(chat(addr, &elsewhere).json()["talos"]["pending"], false);
}

#[test]
fn identity_by_user_field() {
    let addr = start();
    let with = |text: &str, who: &str| {
        let mut body = user(text);
        body["user"] = json!(who);
        body
    };
    assert!(kept(addr, &with("hola", "ana"), &with("hello", "ana")));
    assert!(!kept(addr, &with("hola", "eva"), &with("hello", "luz")));
    // An explicit key wins over `user`.
    let mut keyed = with("hello", "ana");
    keyed["metadata"] = json!({ "conversation": "other" });
    assert!(is_english_greeting(&chat(addr, &keyed).content()));
}

#[test]
fn identity_by_transcript_fingerprint() {
    let addr = start();
    let first = user("hola");
    let reply = chat(addr, &first).content();
    assert_eq!(reply, "¡Hola!");
    // The client resends the history with the reply, as OpenAI clients do;
    // system messages and content given as parts do not change it.
    let second = json!({
        "model": "talos",
        "messages": [
            { "role": "system", "content": "You are a helpful assistant." },
            { "role": "user", "content": [{ "type": "text", "text": "hola" }] },
            { "role": "assistant", "content": reply },
            { "role": "user", "content": "hello" },
        ],
    });
    let reply2 = chat(addr, &second).content();
    assert!(reply2.starts_with("¡Hola"), "{reply2}");
    // And the third turn finds the second.
    let mut third = second.clone();
    let messages = third["messages"].as_array_mut().unwrap();
    messages.push(json!({ "role": "assistant", "content": reply2 }));
    messages.push(json!({ "role": "user", "content": "hello" }));
    assert!(chat(addr, &third).content().starts_with("¡Hola"));
    // An edited history is not found: the turn is treated as fresh.
    let edited = json!({
        "model": "talos",
        "messages": [
            { "role": "user", "content": "hola" },
            { "role": "assistant", "content": "something Talos never said" },
            { "role": "user", "content": "hello" },
        ],
    });
    assert!(is_english_greeting(&chat(addr, &edited).content()));
}

#[test]
fn a_token_is_required_when_configured() {
    let addr = start_with(Config::default(), |s| s.token = Some("s3cret".to_owned()));
    let body = user("hola").to_string();
    let mut conn = Conn::open(addr);
    let none = conn.request("POST", "/v1/chat/completions", &[], &body);
    let error = error_of(&none, 401);
    assert_eq!(error["code"], "invalid_api_key");
    assert_eq!(none.header("www-authenticate"), Some("Bearer"));
    let wrong = conn.request(
        "POST",
        "/v1/chat/completions",
        &[("Authorization", "Bearer nope")],
        &body,
    );
    error_of(&wrong, 401);
    let right = conn.request(
        "POST",
        "/v1/chat/completions",
        &[("Authorization", "bearer s3cret")],
        &body,
    );
    assert_eq!(right.status, 200);
    error_of(&conn.request("GET", "/health", &[], ""), 401);
    let health = conn.request("GET", "/health", &[("Authorization", "Bearer s3cret")], "");
    assert_eq!(health.status, 200);
}

#[test]
fn a_non_loopback_bind_needs_a_token() {
    let engine = || talos_skills::engine(Config::default()).unwrap();
    let mut open = settings();
    open.addr = "0.0.0.0:0".parse().unwrap();
    assert!(matches!(
        Server::bind(engine(), open.clone()),
        Err(StartError::TokenRequired(_))
    ));
    open.addr = "[::]:0".parse().unwrap();
    assert!(matches!(
        Server::bind(engine(), open),
        Err(StartError::TokenRequired(_))
    ));
    let mut bad = settings();
    bad.token = Some("two words".to_owned());
    assert!(matches!(
        Server::bind(engine(), bad),
        Err(StartError::BadToken)
    ));
    // A token on loopback is allowed (and then required).
    let mut fine = settings();
    fine.token = Some("t".to_owned());
    assert!(Server::bind(engine(), fine).is_ok());
}

#[test]
fn bad_requests_get_openai_shaped_errors() {
    let addr = start_with(Config::default(), |s| s.limits.max_body = 4_096);
    let post = |body: &str| Conn::open(addr).request("POST", "/v1/chat/completions", &[], body);
    let error = error_of(&post("{not json"), 400);
    assert_eq!(error["type"], "invalid_request_error");
    error_of(&post(""), 400);
    error_of(&post("[]"), 400);
    assert_eq!(error_of(&post("{}"), 400)["param"], "messages");
    let no_user = json!({ "messages": [{ "role": "system", "content": "be brief" }] });
    assert_eq!(
        error_of(&post(&no_user.to_string()), 400)["param"],
        "messages"
    );
    let no_role = json!({ "messages": [{ "content": "hola" }] });
    assert_eq!(
        error_of(&post(&no_role.to_string()), 400)["param"],
        "messages[0].role"
    );
    let bad_content = json!({ "messages": [{ "role": "user", "content": 7 }] });
    error_of(&post(&bad_content.to_string()), 400);
    let mut bad_lang = user("hola");
    bad_lang["metadata"] = json!({ "language": "fr" });
    assert_eq!(
        error_of(&post(&bad_lang.to_string()), 400)["param"],
        "metadata.language"
    );
    let mut bad_stream = user("hola");
    bad_stream["stream"] = json!("yes");
    error_of(&post(&bad_stream.to_string()), 400);

    let huge = json!({ "messages": [{ "role": "user", "content": "x".repeat(5_000) }] });
    let mut conn = Conn::open(addr);
    let reply = conn.request("POST", "/v1/chat/completions", &[], &huge.to_string());
    assert_eq!(error_of(&reply, 413)["code"], "request_too_large");
    assert_eq!(reply.header("connection"), Some("close"));

    let unknown = get(addr, "/v1/completions");
    assert_eq!(error_of(&unknown, 404)["code"], "not_found");
    let wrong = get(addr, "/v1/chat/completions");
    error_of(&wrong, 405);
    assert_eq!(wrong.header("allow"), Some("POST"));
    let wrong = Conn::open(addr).request("POST", "/health", &[], "");
    error_of(&wrong, 405);
    assert_eq!(wrong.header("allow"), Some("GET"));

    let mut conn = Conn::open(addr);
    conn.send_raw(b"NOT A REQUEST\r\n\r\n");
    error_of(&conn.read().unwrap(), 400);
    assert!(conn.closed());
    let mut conn = Conn::open(addr);
    conn.send_raw(b"GET /health HTTP/2.0\r\n\r\n");
    error_of(&conn.read().unwrap(), 505);
}

#[test]
fn what_real_clients_send_is_accepted() {
    let addr = start();
    // The shape of an OpenAI SDK request, with everything Talos ignores.
    let body = json!({
        "model": "gpt-4o-mini",
        "messages": [
            { "role": "developer", "content": "Answer briefly." },
            { "role": "system", "content": [{ "type": "text", "text": "Be kind." }] },
            { "role": "user", "content": [
                { "type": "text", "text": "hola" },
                { "type": "image_url", "image_url": { "url": "data:image/png;base64,AAAA" } },
            ] },
        ],
        "temperature": 0.2,
        "top_p": 1,
        "n": 1,
        "max_tokens": 64,
        "max_completion_tokens": 64,
        "presence_penalty": 0,
        "frequency_penalty": 0,
        "logit_bias": {},
        "seed": 42,
        "stop": ["\n\n"],
        "stream": false,
        "stream_options": null,
        "tools": [{ "type": "function", "function": {
            "name": "get_weather",
            "parameters": { "type": "object", "properties": {} },
        } }],
        "tool_choice": "auto",
        "parallel_tool_calls": true,
        "response_format": { "type": "text" },
        "metadata": { "client": "sdk" },
        "store": false,
        "something_new": { "nested": [1, 2, 3] },
    });
    let headers = [
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
        ("User-Agent", "OpenAI/Python 1.0"),
        ("X-Stainless-Lang", "python"),
        ("Authorization", "Bearer sk-not-needed"),
    ];
    let reply =
        Conn::open(addr).request("POST", "/v1/chat/completions", &headers, &body.to_string());
    assert_eq!(reply.status, 200, "{}", reply.text());
    assert_eq!(reply.content(), "¡Hola!");

    // A tool-call turn in the history (assistant content null, a tool
    // message) does not stop the next user message from being answered.
    let body = json!({
        "messages": [
            { "role": "user", "content": "weather?" },
            { "role": "assistant", "content": null, "tool_calls": [{
                "id": "c1", "type": "function",
                "function": { "name": "get_weather", "arguments": "{}" },
            }] },
            { "role": "tool", "tool_call_id": "c1", "content": "sunny" },
            { "role": "user", "content": "hello" },
        ],
    });
    assert_eq!(chat(addr, &body).status, 200);

    // curl sends `Expect: 100-continue` for larger bodies; some clients
    // send chunked bodies.
    let body = user("hola").to_string();
    let mut conn = Conn::open(addr);
    conn.send_raw(
        format!(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: t\r\nExpect: 100-continue\r\n\
             Content-Length: {}\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    assert_eq!(conn.read().unwrap().status, 100);
    conn.send_raw(body.as_bytes());
    assert_eq!(conn.read().unwrap().content(), "¡Hola!");
    let (a, b) = body.split_at(10);
    conn.send_raw(
        format!(
            "POST /v1/chat/completions HTTP/1.1\r\nHost: t\r\nTransfer-Encoding: chunked\r\n\r\n\
             {:x}\r\n{a}\r\n{:x}\r\n{b}\r\n0\r\n\r\n",
            a.len(),
            b.len()
        )
        .as_bytes(),
    );
    assert_eq!(conn.read().unwrap().content(), "¡Hola!");
}

#[test]
fn connections_are_kept_alive_until_closed() {
    let addr = start();
    let mut conn = Conn::open(addr);
    for _ in 0..5 {
        assert_eq!(conn.request("GET", "/health", &[], "").status, 200);
    }
    let last = conn.request("GET", "/health", &[("Connection", "close")], "");
    assert_eq!(last.header("connection"), Some("close"));
    assert!(conn.closed());
    let mut old = Conn::open(addr);
    old.send_raw(b"GET /health HTTP/1.0\r\n\r\n");
    assert_eq!(old.read().unwrap().status, 200);
    assert!(old.closed());
}

#[test]
fn the_dialog_state_store_is_bounded() {
    let config = Config {
        max_conversations: 2,
        ..Config::default()
    };
    let addr = start_with(config, |_| {});
    let with = |text: &str, key: &str| {
        let mut body = user(text);
        body["metadata"] = json!({ "conversation": key });
        body
    };
    for key in ["a", "b", "c"] {
        assert_eq!(chat(addr, &with("hola", key)).content(), "¡Hola!");
    }
    // "a" was the least recently used: it was forgotten when "c" came.
    assert!(is_english_greeting(
        &chat(addr, &with("hello", "a")).content()
    ));
    // "c" is still held ("b" went to make room for "a" again).
    assert!(
        chat(addr, &with("hello", "c"))
            .content()
            .starts_with("¡Hola")
    );
    assert!(is_english_greeting(
        &chat(addr, &with("hello", "b")).content()
    ));

    // The transcript table has the same bound.
    let addr = start_with(config, |_| {});
    let history = |first: &str, reply: &str| {
        json!({ "messages": [
            { "role": "user", "content": first },
            { "role": "assistant", "content": reply },
            { "role": "user", "content": "hello" },
        ] })
    };
    let replies: Vec<String> = ["hola", "buenas", "buenos días"]
        .iter()
        .map(|t| chat(addr, &user(t)).content())
        .collect();
    let oldest = chat(addr, &history("hola", &replies[0])).content();
    assert!(is_english_greeting(&oldest), "{oldest}");
    let newest = chat(addr, &history("buenos días", &replies[2])).json();
    assert_eq!(newest["talos"]["language"], "es", "{newest}");
}

#[test]
fn two_clients_at_once() {
    let addr = start();
    let client = |key: &'static str, text: &'static str| {
        std::thread::spawn(move || {
            let mut conn = Conn::open(addr);
            let mut replies = Vec::new();
            for _ in 0..50 {
                let mut body = user(text);
                body["metadata"] = json!({ "conversation": key });
                replies.push(conn.chat(&body).json()["talos"]["language"].clone());
            }
            replies
        })
    };
    let es = client("es", "hola");
    let en = client("en", "hello there");
    assert!(es.join().unwrap().iter().all(|l| l == "es"));
    assert!(en.join().unwrap().iter().all(|l| l == "en"));
}

#[test]
fn a_slow_client_does_not_block_others() {
    let addr = start_with(Config::default(), |s| {
        s.limits.read_timeout = Duration::from_millis(300);
    });
    // Half a request, then silence.
    let mut slow = Conn::open(addr);
    slow.send_raw(b"POST /v1/chat/completions HTTP/1.1\r\nHost: t\r\n");
    // An idle connection that sends nothing at all.
    let mut idle = Conn::open(addr);
    let started = Instant::now();
    assert_eq!(chat(addr, &user("hola")).status, 200);
    assert!(started.elapsed() < Duration::from_millis(250));
    // The slow one is told it took too long; the idle one is just closed.
    error_of(&slow.read().unwrap(), 408);
    assert!(idle.read().is_none());
}

#[test]
fn connections_over_the_limit_are_refused() {
    let addr = start_with(Config::default(), |s| s.limits.max_connections = 1);
    let mut first = Conn::open(addr);
    assert_eq!(first.request("GET", "/health", &[], "").status, 200);
    let mut second = Conn::open(addr);
    let refused = second.read().unwrap();
    assert_eq!(error_of(&refused, 503)["type"], "server_error");
    assert!(second.closed());
    drop(first);
    // Once the first has gone, a new one is served.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut again = Conn::open(addr);
        again.send_raw(b"GET /health HTTP/1.1\r\nHost: t\r\n\r\n");
        if again.read().unwrap().status == 200 {
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The bodies of a fixed request sequence, from a fresh server.
fn session() -> Vec<Vec<u8>> {
    let addr = start_with(
        Config {
            seed: 7,
            ..Config::default()
        },
        |_| {},
    );
    let mut bodies = Vec::new();
    let mut conn = Conn::open(addr);
    let mut keyed = |text: &str, stream: bool| {
        let mut body = user(text);
        body["metadata"] = json!({ "conversation": "det" });
        body["stream"] = json!(stream);
        conn.chat(&body).body
    };
    bodies.push(keyed("hola", false));
    bodies.push(keyed("tira un dado", false));
    bodies.push(keyed("tira una moneda", true));
    bodies.push(keyed("creo que el cine es aburrido", false));
    let first = chat(addr, &user("hello"));
    bodies.push(first.body.clone());
    let reply = first.content();
    let next = json!({ "messages": [
        { "role": "user", "content": "hello" },
        { "role": "assistant", "content": reply },
        { "role": "user", "content": "roll two dice" },
    ] });
    bodies.push(chat(addr, &next).body);
    let mut by_user = user("pick a number between 1 and 100");
    by_user["user"] = json!("someone");
    bodies.push(chat(addr, &by_user).body);
    bodies
}

#[test]
fn the_same_requests_give_the_same_bytes() {
    let (a, b) = (session(), session());
    assert_eq!(a.len(), b.len());
    for (x, y) in a.iter().zip(&b) {
        assert_eq!(String::from_utf8_lossy(x), String::from_utf8_lossy(y));
    }
}

/// A data directory of the test's own, so the binary never reads or writes
/// the user's real one (docs/DEVELOPMENT.md, "User data").
fn scratch_data_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("talos-server-test-{}", std::process::id()))
}

#[test]
fn the_binary_serves_and_refuses_an_open_bind_without_a_token() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(["serve", "--port", "0"])
        .env_remove("TALOS_TOKEN")
        .env("TALOS_DATA_DIR", scratch_data_dir())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut line = String::new();
    stderr.read_line(&mut line).unwrap();
    let addr: SocketAddr = line
        .trim()
        .rsplit("http://")
        .next()
        .unwrap()
        .parse()
        .unwrap_or_else(|_| panic!("{line}"));
    assert!(addr.ip().is_loopback());
    let health = get(addr, "/health");
    child.kill().unwrap();
    child.wait().unwrap();
    assert_eq!(health.status, 200);

    let output = Command::new(env!("CARGO_BIN_EXE_talos"))
        .args(["serve", "--bind", "0.0.0.0", "--port", "0"])
        .env_remove("TALOS_TOKEN")
        .env("TALOS_DATA_DIR", scratch_data_dir())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(64));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("without a token"), "{message}");
}
