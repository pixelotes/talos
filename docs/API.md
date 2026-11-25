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
talos --version

talos serve                     HTTP server                       (planned)
talos eval [battery]            run an evaluation battery         (planned)
talos packs list | add | remove manage knowledge packs            (planned)
talos teach export | import     back up what the user has taught  (planned)
```

Built today: `ask`, `chat`, `explain` and `skills list | check`; the rest of
the list is planned. Flags, before or after the subcommand: `--lang es|en`
(the caller's hint), `--seed N` (the random source's seed, default 0) and
`--json` (for `ask` and `explain`: print the full outcome object instead of
the reply text). `--config PATH` and `--data-dir PATH` are planned with
`talos.toml`.

`talos chat` reads one message per line from standard input (a `> ` prompt
only on a terminal) and prints one reply per line until end of input; all its
lines are one conversation.

Exit status of `talos ask`: `0` answered, `2` deflected, `3` unhandled. A
shell script can fall through to something else exactly as a host application
would. Other statuses: `talos skills check` exits `1` when a manifest has
errors; `64` for wrong arguments, `70` when the engine cannot be built, `74`
when reading or writing the terminal fails. `talos explain` exits `0`.

`--json` prints one object with the fields of the `talos` object of §2.1,
plus the reply:

```json
{ "reply": "…", "status": "answered", "intent": "smalltalk.greet",
  "slots": {}, "confidence": 1.0, "language": "es", "reason": null,
  "pending": false, "generated": false, "elapsed_us": 412 }
```

Slot values are strings (a list is an array of strings; numbers keep their
exact decimal text, dates are ISO 8601). `intent` and `reason` are `null` when
absent. With `explain`, the object also carries `trace` (an array of
`{stage, note}`) and `internal_slots`. Without `--json`, `explain` prints one
line per stage, then `outcome` and `reply`.

## 2. HTTP server

`talos serve` binds `127.0.0.1:8099` by default. Binding to any other address
requires a bearer token in configuration.

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | liveness, version, loaded skills and packs, failures |
| `GET` | `/v1/models` | OpenAI-compatible; lists a single model, `talos` |
| `POST` | `/v1/chat/completions` | OpenAI-compatible chat |

### 2.1 `POST /v1/chat/completions`

Accepts the standard request. Talos reads:

- the **last user message** as the turn text;
- `stream` — when true, replies as server-sent events in the standard chunk
  format;
- optional hints, all ignorable by a standard client:
  `metadata.language`, `metadata.conversation`, `metadata.place`,
  `metadata.trace`.

Sampling parameters (`temperature`, `top_p`, …), tools and system prompts are
accepted and ignored: Talos is deterministic and has no prompt.

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

The same status is sent as an `X-Talos-Status` header, so a proxy can decide
without parsing the body. Clients that know nothing about Talos simply see a
chat completion.

### 2.2 What a host reads

There is no separate native endpoint. A host that places Talos in front of
another engine uses the same chat-completions call as any client and reads the
`talos` object (or the `X-Talos-Status` header):

| Field | Meaning |
|---|---|
| `status` | `answered`, `deflected` or `unhandled` — whether to show the reply or pass the turn on |
| `pending` | Talos asked a question back; send the next message of this conversation to Talos first |
| `generated` | true only when the answer came from an external skill that declares it uses a model; every other outcome is reproducible |
| `intent`, `slots`, `confidence`, `language` | what was recognised, for logging and routing decisions; `intent` is kept when the status is `unhandled` |
| `reason` | why a turn is `unhandled` (unknown, cannot, generative, noise, other language…) or which shape was `deflected` |

Hints travel the other way in `metadata` (`language`, `conversation`, `place`,
`trace`). This is the whole integration contract: one standard endpoint, one
additive object. Talos and its host share no bus, no process and no code.

### 2.3 Conversation identity

Dialog state (an open question, a game in progress) is kept server-side per
conversation. The conversation is identified, in order of preference, by:

1. `metadata.conversation` in the body, or the `X-Talos-Conversation` header;
2. the `user` field of the OpenAI request;
3. a **transcript fingerprint**: OpenAI-style clients resend the whole
   history each turn, so the hash of every message before the last one
   identifies where the conversation is. After answering, Talos stores the
   state under the fingerprint the *next* request will have.

The third option lets unmodified OpenAI clients hold a multi-turn dialog with
no configuration. It breaks if the client edits or truncates history; then the
state is simply not found and the turn is treated as fresh. This scheme is an
open question to validate — see [DESIGN.md](DESIGN.md).

In-memory dialog state is bounded (a maximum number of conversations, an idle
expiry) so the server's memory cannot grow without limit.

### 2.4 Errors

Standard HTTP status codes with an OpenAI-shaped error body. A turn Talos
cannot answer is **not** an error: it is a `200` with `status: "unhandled"`.

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
