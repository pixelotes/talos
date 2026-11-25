# 0017. The HTTP server is a small HTTP/1.1 implementation on `std`

- Status: Accepted
- Date: 2026-11-25

## Context

`talos serve` ([0010](0010-one-binary-three-front-ends.md), docs/API.md §2)
answers three routes for one local client or a handful: `GET /health`,
`GET /v1/models`, `POST /v1/chat/completions`, the last also as server-sent
events. DESIGN.md §9 listed `axum` on a single-threaded `tokio` runtime as the
candidate, undecided, behind a `server` feature.

What the server has to do is narrow:

- The engine is synchronous and a turn costs tens of microseconds (a rule)
  to about a millisecond (the reflector). Nothing waits on I/O inside a turn.
- A reply exists in full before the first byte is sent, so "streaming" is a
  framing of one known body, not a stream.
- It must bound request sizes, time out reads and writes, keep connections
  alive or close them cleanly, and not let a slow or half-open client hold
  up the others.
- Budgets (ROADMAP v0.4, DESIGN.md §8): server overhead ≤ 1 ms per request,
  resident memory with the server ≤ 12 MB at rest without packs, the binary
  ≤ 6 MB without the server feature and ≤ 15 MB in all.
- Policy (DESIGN.md §9): no dependency, then a small one, then a
  feature-gated one; permissive licences only.

### Measurements

Three servers returning the same 450-byte JSON body, built with the
workspace's release profile (`opt-level = "z"`, fat LTO, stripped,
`panic = "abort"`), in `spikes/http-stack/` (`measure.sh` reproduces the
table). Apple-silicon Mac, macOS, loopback; round trips are 10 000 POSTs
over one keep-alive connection from a plain `std` client; resident set from
`ps -o rss`.

| | `std`, thread per connection | `tiny_http` 0.12 | `axum` 0.8 + `tokio` 1 (current-thread, `http1` only) |
|---|---|---|---|
| stripped binary (hello server alone) | 320 KB | 404 KB | 554 KB |
| crates added | 0 | 5 (`tiny_http`, `ascii`, `chunked_transfer`, `httpdate`, `log`) | 36 (`hyper`, `hyper-util`, `tokio`, `mio`, `tower`, `http`, `futures-*`, …, and `tokio-macros` with `syn` and `quote` at build time) |
| RSS at rest | 1.7 MB | 1.9 MB | 2.0 MB |
| RSS after 1 000 / 11 000 requests | 1.9 / 1.9 MB | 2.2 / 2.2 MB | 2.4 / 2.4 MB |
| round trip p50 / p99 | 31 / 62 µs | 29 / 45 µs | 25 / 32 µs |

All three are far inside every budget; latency differences are a few
microseconds against a 1 ms budget. What separates them is what each brings
and what each still leaves to be written:

- **`axum` + `tokio`**: 36 crates and the largest binary, for an async
  runtime the engine cannot use (a turn would run on the runtime's only
  thread, or be moved to a blocking pool). Body limits exist
  (`DefaultBodyLimit`), but header-read and idle timeouts and a connection
  cap mean configuring `hyper`'s connection builder below `axum::serve` or
  adding `tower-http` layers: more code or more crates.
- **`tiny_http`**: small and permissive (MIT OR Apache-2.0), but it sets no
  socket timeouts, has no configurable limit on header or body size, and its
  worker pool grows without bound; requests arrive on a single queue that
  the application must drain and parallelise itself. Every guard the server
  needs would be written on top of it anyway.
- **`std`**: the guards are the code. HTTP/1.1 for three routes with
  `Content-Length` responses is a few hundred lines: a bounded line reader
  with a deadline, `Content-Length` and chunked bodies, `Expect:
  100-continue`, keep-alive.

## Decision

Write the server on `std` alone, in `crates/talos/src/server/`, behind the
`server` feature of the `talos` crate. The feature is **on by default** and
adds no dependency; `cargo build --no-default-features` gives a binary without
a listener, so the "binary without the server feature" budget stays
measurable.

- **Threads.** The listener accepts on the main thread and serves each
  connection on a thread of its own, at most 64 at once; one more is answered
  `503` and closed. A slow client holds its own thread only. The engine is
  shared by reference (`Engine` is `Sync`; one lock per conversation).
- **Limits.** Body ≤ 1 MiB, request line and headers ≤ 16 KiB and ≤ 100
  lines, read and write timeouts of 10 s (also the keep-alive idle time), and
  30 s for a whole request once it has started, checked between reads, so a
  client trickling a byte at a time cannot hold a connection past it.
- **Framing.** Requests: `Content-Length` or `chunked`; other transfer
  encodings are `501`. Responses always carry `Content-Length`, the
  server-sent events too: the whole event stream is known before it is
  sent. HTTP/1.0 is answered and closed; HTTP/1.1 is kept alive unless the
  client says `Connection: close`. After an error response to a request not
  read in full, the server stops sending and drains the rest briefly before
  closing, so the client is not reset before it reads the error.
- **JSON** stays `serde_json` (already in the binary for `--json`).
- **Clock.** The server reads the local UTC offset at most once a minute
  (`LocalClock`), not on every request: running `date` costs more than the
  rest of a request.

The same code measured as Talos (`talos serve`, the real engine, release
build, same machine; `examples/serve_load.rs` and `spikes/http-stack/measure.sh`):

| | value |
|---|---|
| stripped binary without / with `server` | 1 400 864 B / 1 468 192 B (+66 KiB; HEAD before this work: 1 400 832 B) |
| crates in the binary's tree, without / with | 23 / 23 (20 third-party) |
| RSS at rest (started, no request yet) | 5.5–6.3 MB |
| RSS after 1 000 one-message requests, all `hola` (1 000 conversations) | 7.1–7.6 MB; 7.3–7.6 MB after 11 000 |
| RSS after 1 000 mixed requests (rule and reflector, both languages) | 9.0–9.4 MB, unchanged after 10 000 more |
| server overhead per request (round trip minus the engine's `elapsed_us`), p50 / p99 | 64 / 122–152 µs |

## Consequences

- No new crate, no async runtime anywhere in the tree; `talos-core` stays free
  of HTTP (the server is in the binary crate).
- The HTTP code is ours to maintain and to get right; it is small, covered
  by unit tests of the parser and by integration tests over real sockets
  (limits, timeouts, slow and idle clients, the connection cap, keep-alive,
  chunked bodies, `100-continue`).
- A thread per connection costs one thread stack per open connection; with
  the cap of 64 that is bounded. It is the wrong shape for thousands of
  concurrent clients, which is not what Talos is for.
- No TLS: the server is for loopback or a trusted network behind a proxy,
  with a bearer token off loopback. HTTP/2 is not served.
- If a future need (WebSockets, TLS in process, many concurrent streams)
  outgrows this, the server is one module behind a feature and can be
  replaced without touching the engine; that would be a new record.

## Alternatives considered

- **`axum` on a current-thread `tokio`** (the earlier candidate): the most
  complete and fastest by a few microseconds, but 36 crates and an async
  runtime for a synchronous engine, and its timeouts still need more layers.
  No budget forces it.
- **`tiny_http`**: 5 small crates, but the limits, timeouts and concurrency
  bounds would all be written on top of it, so it saves little code for a
  dependency.
- **`hyper` alone, `may_minihttp`, `astra`, `rouille`**: between the two
  above in size, each with an async runtime, a coroutine runtime, or
  `tiny_http` underneath. Not measured.
- **A fixed worker pool with a queue**: bounds threads, but a slow client
  then occupies a worker and delays others queued behind it; a thread per
  connection with a cap does not have that coupling.
