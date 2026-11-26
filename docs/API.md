# Interfaces

One binary, `talos`, with three ways in. All three call the same engine
function and return the same outcome. Decision:
[ADR 0010](adr/0010-one-binary-three-front-ends.md).

## 1. Command line

```
talos chat                      interactive session
talos ask "what time is it"     one answer, then exit
talos explain "…"               show the pipeline trace for a message
talos skills list | check       inspect and validate skill manifests
talos eval [--dir evals]        score the intent batteries, rules only
talos serve                     HTTP server (§2)
talos --version

talos packs list | add | remove manage knowledge packs            (planned)
talos teach export | import     back up what the user has taught  (planned)
```

Built today: `ask`, `chat`, `explain`, `skills list | check`, `eval` and
`serve`; the rest of the list is planned. Flags, before or after the subcommand: `--lang es|en`
(the caller's hint), `--seed N` (the random source's seed, default 0) and
`--json` (for `ask` and `explain`: print the full outcome object instead of
the reply text) and `--net` (let the skills that need the network use it;
the same as `TALOS_NET=1`. Without it they answer that they have no network
access. `TALOS_CONTACT`, an address or a page, is added to the client's
User-Agent, `talos/<version>`, when set: Wikipedia's policy for automated
clients asks for one and limits clients without it). `--config PATH` and `--data-dir PATH` are planned with
`talos.toml`. `serve` also takes `--bind ADDR` (an IP address or
`localhost`), `--port N` (`0` picks a free port) and `--token TOKEN`; with
`serve`, `--lang` is the hint for requests that give none.

`talos chat` reads one message per line from standard input (a `> ` prompt
only on a terminal) and prints one reply per line until end of input; all its
lines are one conversation.

A message that begins with `/` is a command (`/calc 1 + 2`, `/ayuda`;
[ADR 0016](adr/0016-command-shortcuts.md)). The engine handles it, so it
works the same through `ask`, `chat` and `serve`, and a host forwards it like
any other text.

Exit status of `talos ask`: `0` answered, `2` deflected, `3` unhandled. A
shell script can fall through to something else exactly as a host application
would. Other statuses: `talos skills check` exits `1` when a manifest has
errors; `64` for wrong arguments, `70` when the engine cannot be built, `74`
when reading or writing the terminal fails. `talos explain` exits `0`.
`talos serve` runs until stopped; it exits `64` when asked to bind a
non-loopback address without a token, `74` when the address cannot be bound.

`talos eval [--dir PATH] [--json]` runs `intents.{es,en}.jsonl` and
`reject.{es,en}.jsonl` from `PATH` (default `evals`) with the engine as it is,
rules only, each row as a fresh single-turn conversation with the row's
language as the hint. Per language it reports the rows, the intent accuracy
(the outcome's intent equals the label, whatever the status), the false
accepts of the rejection set (`answered` with an intent, also per `kind`),
the share of rejection rows with the expected status, and each intent's rows
and correct ones, worst first. `--json` prints the same as one object. It is a
baseline, not a gate: it exits `0` whatever the scores, `74` when a battery
cannot be read.

`--json` prints one object with the fields of the `talos` object of §2.1,
plus the reply:

```json
{ "reply": "…", "status": "answered", "intent": "smalltalk.greet",
  "slots": {}, "confidence": 1.0, "language": "es", "reason": null,
  "pending": false, "generated": false, "elapsed_us": 412 }
```

Slot values are strings (a list is an array of strings; numbers keep their
exact decimal text, dates are ISO 8601). `intent` and `reason` are `null` when
absent; `fallback` is present only when the reply rests on a cheaper source
(§2.2). With `explain`, the object also carries `trace` (an array of
`{stage, note}`) and `internal_slots`. Without `--json`, `explain` prints one
line per stage, then `outcome` and `reply`.

## 2. HTTP server

`talos serve` binds `127.0.0.1:8099` by default and prints the address it
serves on to standard error. Binding to any other address requires a bearer
token: `--token TOKEN`, or the `TALOS_TOKEN` environment variable (preferred:
a flag is visible in the process list). Without one the server refuses to
start. A token given on loopback is required too. With a token, **every**
request, `/health` included, must carry `Authorization: Bearer TOKEN`;
otherwise `401` with `WWW-Authenticate: Bearer`. The server is compiled only
with the `server` feature of the `talos` crate (on by default;
[ADR 0017](adr/0017-http-stack.md)).

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | liveness, version, loaded skills and packs, failures |
| `GET` | `/v1/models` | OpenAI-compatible; lists a single model, `talos` |
| `GET` | `/v1/models/talos` | that model alone; any other id is `404` |
| `POST` | `/v1/chat/completions` | OpenAI-compatible chat |

`/health` answers
`{"status": "ok", "version": "…", "skills": ["calc", …], "packs": [{"id": "core", "version": "1.0", "languages": ["es", "en"]}], "failures": [{"pack": "/…/packs/old.talospack", "error": "built for schema version 0; …"}]}`:
`packs` are the attached knowledge packs in the order they are asked;
`failures` the packs found in the data directory and left out, with the
reason (external skills will join them). Both are empty without packs, and
in a build without the `knowledge` feature.

HTTP/1.1 (and 1.0) with keep-alive. Request bodies come with
`Content-Length` or chunked; `Expect: 100-continue` is honoured. Every
response, streamed ones included, carries a `Content-Length`. Limits: a body
of at most 1 MiB, a request line plus headers of at most 16 KiB and 100
header lines, 64 connections at once (one more is answered `503` and closed),
10 s for any read or write (also how long an idle connection is kept) and
30 s for a whole request to arrive.

### 2.1 `POST /v1/chat/completions`

Accepts the standard request. Talos reads:

- the **last user message** as the turn text. Its `content` is a string, an
  array of parts (the `text` parts are joined with newlines; image, audio
  and other parts are ignored) or null (an empty turn). Messages after it, if
  any, are ignored;
- `stream` — when true, replies as server-sent events in the standard chunk
  format; `stream_options.include_usage` is honoured;
- `user`, for conversation identity (§2.3);
- optional hints, all ignorable by a standard client:
  `metadata.language` (`es` or `en`, anything else is `400`; when absent,
  the server's `--lang`, if given), `metadata.conversation` (§2.3),
  `metadata.place` (passed to the turn as the caller's place) and
  `metadata.trace` (when true, the `talos` object also carries `trace` and
  `internal_slots`, as `talos explain --json` does). OpenAI metadata values
  are strings, so `"true"` and `"false"` are accepted for booleans.

Everything else is accepted and ignored, including fields Talos does not
know: `model` (the response always says `talos`), sampling parameters
(`temperature`, `top_p`, `seed`, …), `max_tokens`, `n` (there is always one
choice), `tools`, `tool_choice`, `response_format`, and system, developer and
tool messages. Talos is deterministic and has no prompt. A field Talos does
read must have the right type (`stream` a boolean, `metadata` an object,
`messages` an array of objects with a string `role`), or the request is `400`.

The response is a standard completion with one extra object:

```json
{
  "id": "talos-…",
  "object": "chat.completion",
  "model": "talos",
  "choices": [{
    "index": 0,
    "message": { "role": "assistant", "content": "Son las 18:42." },
    "finish_reason": "stop"
  }],
  "usage": { "prompt_tokens": 0, "completion_tokens": 0, "total_tokens": 0 },
  "talos": {
    "status": "answered",
    "intent": "clock.now",
    "confidence": 1.0,
    "language": "es",
    "slots": {},
    "reason": null,
    "pending": false,
    "generated": false,
    "elapsed_us": 412
  }
}
```

`id` is `talos-` and 16 hexadecimal digits, counting the completions the
server has answered; `created` is the turn's `now`. Each choice also has
`"logprobs": null`. `elapsed_us` is the engine's time for the turn, not the
request's. Object keys are in alphabetical order.

The same status is sent as an `X-Talos-Status` header, so a proxy can decide
without parsing the body. Clients that know nothing about Talos simply see a
chat completion.

Streamed, the body is `text/event-stream`: one `data: {chunk}` event with
`delta: {"role": "assistant", "content": ""}`, one with the whole reply as
`delta.content` (Talos has the reply before the first byte is sent), one with
an empty `delta` and `finish_reason: "stop"` that also carries the `talos`
object, then, with `include_usage`, a chunk with `choices: []` and the
(zero) usage (and `"usage": null` on the others), then `data: [DONE]`. Each
chunk is a `chat.completion.chunk` with the same `id`, `created` and
`model`. The `X-Talos-Status` header is sent with streams too.

### 2.2 What a host reads

There is no separate native endpoint. A host that places Talos in front of
another engine uses the same chat-completions call as any client and reads the
`talos` object (or the `X-Talos-Status` header):

| Field | Meaning |
|---|---|
| `status` | `answered`, `deflected` or `unhandled` — whether to show the reply or pass the turn on |
| `pending` | a question is open after this turn: Talos asked one back (the status is then `answered` and `intent` the request being completed), or the user hesitated and it stays open. Send the next message of this conversation to Talos first: it may answer, cancel or replace the question. It closes after three more turns or five minutes |
| `generated` | true only when the answer came from an external skill that declares it uses a model; every other outcome is reproducible |
| `fallback` | present only when the reply rests on a cheaper source because the best one had nothing ([DESIGN.md](DESIGN.md) principle 8): `word-pairs` (a translation from the word-pair lexicon, `answered`, hedged in the reply), `word-by-word` (a phrase rendered word by word, `unhandled`), `relations` (no definition in the conversation's language; what the word's claims say instead, `unhandled`), `other-language` (the other language's definition, given because it was asked for; or the other language's Wikipedia, because the conversation's has no such article), `encyclopedia` (a word no dictionary has, answered with the opening of its Wikipedia article). The reply always says so in words too. An open set: later skills add their own (a cached value, an article's opening). A host with something better — a translator, an encyclopedia — can step in. `talos explain` and golden outcome lines end with `· fallback <name>` |
| `intent`, `slots`, `confidence`, `language` | what was recognised, for logging and routing decisions; `intent` is kept when the status is `unhandled` |
| `reason` | why a turn is `unhandled` (unknown, cannot, generative, noise, other language, `nothing-pending` for a bare yes/no with no question open, `unknown-command`, `command-argument`, `gave-up` for a question dropped after two failed answers…) or which shape was `deflected`; on an `answered` turn with no intent, the dialog move: `acknowledgement`, `cancelled`, `hedge` |

Hints travel the other way in `metadata` (`language`, `conversation`, `place`,
`trace`). This is the whole integration contract: one standard endpoint, one
additive object. Talos and its host share no bus, no process and no code.

### 2.3 Conversation identity

Dialog state (an open question, a game in progress) is kept server-side per
conversation. The conversation is identified, in order of preference, by:

1. `metadata.conversation` in the body, or the `X-Talos-Conversation` header
   (the body wins when both are given);
2. the `user` field of the OpenAI request;
3. a **transcript fingerprint**: OpenAI-style clients resend the whole
   history each turn, so the hash of every message before the last user
   message identifies where the conversation is. After answering, Talos
   stores the state under the fingerprint the *next* request will have
   (this history, the user message and the reply).

An empty key or `user` counts as absent. Keys and `user` values are separate
namespaces; one longer than 128 bytes is replaced by its hash. The
fingerprint is a 128-bit FNV-1a over the **user and assistant** messages
only, each as its role and its text with surrounding whitespace trimmed:
system, developer and tool messages are left out, so a system prompt that
changes from turn to turn (a date in it, say) does not break the
conversation, and a client that trims the reply still matches.

The third option lets unmodified OpenAI clients hold a multi-turn dialog with
no configuration. It breaks if the client edits or truncates history; then the
state is simply not found and the turn is treated as fresh. Earlier messages
are never replayed through the engine: the state comes only from the store.
A client that retries or regenerates a turn sends the same history and so
continues the same conversation, whose state the first attempt has already
advanced. This scheme is an open question to validate — see
[DESIGN.md](DESIGN.md).

What Talos remembers about its user (name, place, units; the correction
table) is not dialog state and is not keyed by any of these: there is one set
per installation, shared by every conversation of the server and by the CLI
(docs/spec/decisions.md ruling 10). A preference set over the API is there
for `talos chat`, and the reverse. It is kept in `user.db` (`user.toml` in a
build without the `knowledge` feature) in the data directory (`$TALOS_DATA_DIR`, else `$XDG_DATA_HOME/talos`, else
`~/.local/share/talos`; `TALOS_DATA_DIR=` empty keeps it for the run only),
written only on a turn that changes it ([DESIGN.md](DESIGN.md) §7).

In-memory dialog state is bounded (a maximum number of conversations, an idle
expiry) so the server's memory cannot grow without limit. The bounds are the
engine's (`max_conversations`, default 1024; `idle_expiry_seconds`, default
30 minutes; the least recently used goes first), and the fingerprint table
has the same ones.

### 2.4 Errors

Standard HTTP status codes with an OpenAI-shaped error body. A turn Talos
cannot answer is **not** an error: it is a `200` with `status: "unhandled"`.

```json
{ "error": { "message": "`messages` must contain a user message",
             "type": "invalid_request_error", "param": "messages", "code": null } }
```

| Status | When | `code` |
|---|---|---|
| `400` | malformed HTTP or JSON, a field Talos reads with the wrong type, no user message | `null` |
| `401` | a token is configured and the request has none or another (`WWW-Authenticate: Bearer`) | `invalid_api_key` |
| `404` | unknown path or model | `not_found` |
| `405` | known path, wrong method (`Allow` says which) | `method_not_allowed` |
| `408` | a request started but did not arrive in time | `timeout` |
| `413` | body over 1 MiB | `request_too_large` |
| `431` | request line and headers over 16 KiB or 100 lines | `null` |
| `501` | a transfer encoding other than `chunked` | `null` |
| `503` | too many connections (`type: server_error`) | `overloaded` |
| `505` | an HTTP version other than 1.0 or 1.1 | `null` |

`type` is `invalid_request_error` except for the `5xx`. After `408`, `413`,
`431` or any malformed request the connection is closed.

## 3. Embedding

The engine is a Rust library. A host written in Rust links `talos-core`
directly and calls `Engine::respond`. Bindings for other languages are out of
scope for the first major version; the HTTP server is the integration path.

## 4. Stability

Until 1.0 everything may change. At 1.0 these become stable and versioned:

- the `talos` extension object in chat completions;
- the skill manifest format;
- the external skill protocol;
- the pack schema.
