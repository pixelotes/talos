# 0010. One binary, three front ends, an OpenAI-compatible server

- Status: Accepted
- Date: 2026-11-24

## Context

Talos must be usable alone at a terminal, scriptable, and reachable by other
programs. Many existing clients and applications already speak the OpenAI
chat-completions protocol, so compatibility with it makes Talos a drop-in
"model" for them. Hosts that route between Talos and another engine need more
than that protocol carries: whether the turn was handled, and whether Talos
is waiting for a reply.

## Decision

Ship a single executable, `talos`, whose engine is a library call
(`respond(turn) -> outcome`) wrapped by three front ends:

- `talos chat` — interactive session;
- `talos ask` — one answer, exit status reflecting the outcome status;
- `talos serve` — HTTP server.

The server offers:

- `POST /v1/chat/completions` and `GET /v1/models`, OpenAI-compatible,
  including streaming. Unknown parameters are accepted and ignored. The
  outcome (status, `pending`, `generated`, intent, confidence) is exposed
  additively, in a `talos` object and an `X-Talos-Status` header.
- `GET /health`.

The HTTP stack is compiled behind a `server` feature. The server binds to
loopback by default and requires a token to bind elsewhere.

Conversation identity for clients that send none is derived from a fingerprint
of the transcript (see [API.md](../API.md)).

### Integration with a host

The OpenAI-compatible endpoint is the **only** integration surface. A host
such as Tomos calls it like any client and reads the `talos` object to decide
whether to show the reply or pass the turn on. There is no native routing
endpoint, no shared event bus, no shared process and no callback from Talos
into the host. An earlier draft of this record had a separate `/v1/route`
endpoint; it was removed before implementation because the extension object
carries everything it did.

## Consequences

- One HTTP interface to implement, document and freeze instead of two.
- Talos and its host evolve independently; either can be replaced.
- Talos cannot borrow the host's tools through the integration. Sharing
  skills is a separate question ([0013](0013-external-skills.md)).

- Any OpenAI-style client can talk to Talos unmodified.
- Hosts get a clean fall-through signal without parsing reply text.
- The engine stays free of HTTP and async; front ends are thin and
  replaceable.
- "OpenAI-compatible" is a moving target. Talos commits to the chat
  completions subset documented in `API.md`, not to the whole API.
- Token counts are meaningless for Talos and reported as zero.
- The transcript-fingerprint scheme is unproven and flagged as an open
  question.

## Alternatives considered

- **Server only.** Makes Talos depend on a client to be useful at all.
- **Separate binaries per mode.** More to build and ship for no gain.
- **Native API only.** Loses the drop-in property that makes Talos easy to
  adopt.
- **gRPC or a custom socket protocol.** Lighter on the wire, heavier on every
  integrator.
