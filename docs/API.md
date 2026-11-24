# Interfaces

One binary, `talos`, with three ways in. All three call the same engine
function and return the same outcome. Decision:
[ADR 0010](adr/0010-one-binary-three-front-ends.md).

## 1. Command line

```
talos chat                      interactive session
talos ask "what time is it"     one answer, then exit
talos serve                     HTTP server

talos explain "…"               show the pipeline trace for a message
talos eval [battery]            run an evaluation battery, print metrics
talos skills list | check       inspect and validate skill manifests
talos packs list | add | remove manage knowledge packs
talos teach export | import     back up what the user has taught
```

Common flags: `--lang es|en`, `--config PATH`, `--data-dir PATH`, `--json`
(print the full outcome object instead of the reply text).

Exit status of `talos ask`: `0` answered, `2` deflected, `3` unhandled. A
shell script can fall through to something else exactly as a host application
would.

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
