# Development

## Prerequisites

- [rustup](https://rustup.rs). `rust-toolchain.toml` pins the stable channel
  with `rustfmt`, `clippy` and the two ARM64 Linux targets; rustup installs
  them on the first `cargo` call in the repo. Minimum Rust version: 1.99.
- Optional: [`cargo-deny`](https://github.com/EmbarkStudios/cargo-deny)
  (`cargo install cargo-deny --locked`) for the licence check.
- Optional, for cross builds: [`zig`](https://ziglang.org) and
  [`cargo-zigbuild`](https://github.com/rust-cross/cargo-zigbuild).

## Commands

```sh
cargo fmt --all --check                                  # formatting
cargo clippy --workspace --all-targets -- -D warnings    # lints
cargo test --workspace                                   # tests
cargo deny check                                         # licences, advisories, bans
cargo build --release                                    # small, stripped binary
TALOS_BLESS=1 cargo test -p talos-skills --test golden   # regenerate golden files
```

Lints are shared through `[workspace.lints]` in `Cargo.toml`
(`unsafe_code` is forbidden). `spikes/` is excluded from the workspace.

## CI

`.github/workflows/ci.yml` runs on every push and pull request:

| Job | Runs |
|---|---|
| `check` | the fmt, clippy and test commands above |
| `deny` | `cargo deny check` against `deny.toml` |
| `release-x86_64` | `cargo build --release --target x86_64-unknown-linux-gnu` |
| `release-aarch64-musl` | `cargo zigbuild` for `aarch64-unknown-linux-musl`, prints the size, fails over budget, uploads the binary |

The binary size budget (bytes, 15 MiB per DESIGN.md section 8) lives only in
`.github/binary-size-budget`.

## Cross-compiling locally

```sh
brew install zig && cargo install cargo-zigbuild --locked   # once
cargo zigbuild --release --target aarch64-unknown-linux-musl
ls -l target/aarch64-unknown-linux-musl/release/talos
```

The musl target gives a static binary that runs on any ARM64 Linux. With no
third-party dependencies yet, `cargo build --target ...` only needs a linker
for the target, which is what zig provides.

## Decisions taken in Slice A

The "decisions to take as work starts" of [plans/v0.1.md](plans/v0.1.md), as
taken, plus the small ones Slice A needed. Slice B may revisit any of them.

| Question | Taken | Note |
|---|---|---|
| Argument parsing | hand-written, in `crates/talos/src/main.rs` | `--lang` before or after the subcommand; usage errors exit 64 |
| Pattern engine | `regex` `RegexSet`, one per language, features `std` + `unicode-perl` only | no `perf` engines: a few dozen anchored patterns do not need them. Superseded after Slice D: see [Footprint](#footprint) |
| Unicode normalisation | `unicode-normalization` (NFKC), then `to_lowercase` | `to_lowercase` stands in for full case folding |
| Manifest embedding | `crates/talos-skills/build.rs` lists `skills/*/manifest.toml` and emits an `include_str!` table | no run-time file access for built-ins |
| Manifest parsing | `serde` + `toml` (parse only), read into a raw form then validated | unknown keys rejected; a summary, example list or reply lacking `es` or `en` is rejected by name |
| Pattern slots | a pattern may set fixed slots (`slots = { daypart = "morning" }`) | lets a handler tell "buenas" from "buenos días" without reading raw text |
| Rule tie-break | priority (higher first), then intent id, then the pattern's position in its manifest | never registration order |
| Mixed language | the turn language's rules first, then the other's (ADR 0011) | so `--lang en "hola"` greets in English |
| Time | `Timestamp` in `talos-core`: Unix seconds plus an optional UTC offset | no date/time crate yet (Slice C); without an offset, time-of-day greetings fall back to a plain hello |
| Turn number | ~~carried on `Turn`~~ owned by the engine since Slice B | see below |
| Reply variants | turn 1 uses the first variant; later turns step through them one at a time from an FNV-1a phase of the conversation id, chosen so that turn 2 differs from turn 1 | one-shot answers are stable; FNV-1a because `std`'s hasher is not stable across releases |
| Fallback | fixed replies in `talos-core/src/fallback.rs` | replaced by the reflector in Slice D |

## Decisions taken in Slice B

| Question | Taken | Note |
|---|---|---|
| Handler signature | `Skill::handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult`, plus `Skill::replies()` listing every reply id the handler can return | `Call` is the request (intent, language, typed slots, original text); `Context` is everything else (clock, place, random source, conversation state, preferences, installed manifests), so the knowledge store joins `Context` later without touching handlers. Contract: [SKILL_AUTHORING.md](SKILL_AUTHORING.md) |
| Accents in patterns | rules match a **match form**: the normalised text with diacritics removed from Latin letters (`qué` → `que`, `mañana` → `manana`); patterns are written without accents or capitals and `talos skills check` rejects any that has them | one consistent rule instead of `qu[eé]` everywhere; language detection still reads the accented text, so accents stay evidence. Captures are read back in the user's original spelling |
| Slots | declared per intent (`[[intent.slot]]`: `name`, `type` = `term`/`number`/`date`/`duration`/`datetime`/`daterange`/`place`/`language`/`choice`/`confirm`, `required`, `repeated`, `internal`); named captures `(?P<name>…)` must be declared slots; extractors turn captures into typed `Value`s before the handler runs | `internal` slots (Slice A's `register`, `daypart`) are left out of `Outcome::slots` and of golden outcome lines unless `Given: slots = all`; a handler may `cx.report(…)` a default it applied ("tira un dado" → `count=1 sides=6`) |
| Numbers | exact decimals (`Number`: `i128` units and a scale), never floats; `Number::format(lang)` writes them by the language's conventions | the separator rules of conversation.md 2.1 are tested as a table |
| Dates | `civil::Date` in-tree (Hinnant's algorithms), no zone rules | the zone database is still Slice C's decision; the golden harness knows a handful of zones the spec uses, with their DST rules, until then |
| Random source | `SplitMix64` in-tree behind `RandomSource`; each turn is seeded from FNV-1a of (configured seed, conversation, turn number, normalised text), lazily, so a turn that draws nothing seeds nothing | handlers draw through `Random` in natural terms (`int`, `scaled`, `coin`, `pick`, `entry`), which is also what golden files script with `Given: draw = …` |
| Conversation state | engine-owned map, one `Mutex` per conversation plus a short-held map lock; at most `Config::max_conversations` (1024), idle expiry `Config::idle_expiry_seconds` (30 min) measured with the injected `now` | holds the turn counter, the language and how firmly it is held, the recent topic (last intent), and the open question (at most one, with the request's slots; it expires three turns or five minutes after it was asked) |
| Language stickiness | a settled conversation switches only on at least two marker words of the other language and more than of its own; a conversation that only defaulted takes the first evidence; a hint settles it | ADR 0015's classifier step waits for v0.2 |
| Greeting prefixes | an intent may be `courtesy = true`; when the whole message matches nothing, a courtesy of up to four words at its start or end is dropped if the rest is recognised | basics.md, "Greeting prefixes"; the reply does not add a greeting of its own |
| Reasons | `Reason` is an open set of names (`empty`, `catch-all`, `cannot`, `failed`, `acknowledgement`, `cancelled`, `hedge`, `nothing-pending`, `gave-up`, `unknown-command`, `command-argument`; the reflector adds its shapes) | outcome lines show the reason when no intent was recognised |
| Local time in the CLI | the binary runs `date +%z` before each turn | the smallest means with no `unsafe` and no crate; honours `TZ`; where `date` is missing the offset is unknown and time-of-day answers stay neutral. The date/time crate is left to Slice C |
| JSON | `serde_json` (std only) in the binary for `--json` | +17 KB on the stripped binary, measured; the server will need it anyway |
| Validator | `talos_core::check`, run by `talos skills check` and by a test in `talos-skills` that requires zero findings | see the list of checks in `check.rs` |

## Footprint

After Slices C and D (seven skills, the reflector), start-up and memory
regressed far past the v0.1 targets (ROADMAP: ≤ 8 MB resident at rest,
rule-matched turn p99 ≤ 2 ms, binary ≤ 6 MB). Measured on an Apple-silicon
Mac, release build:

| | before | after |
|---|---|---|
| `talos ask "hola"`: wall / max RSS | ~100 ms / 20.7 MB | ~20 ms / 6.8 MB |
| one process after a rule and a reflector turn in each language: max RSS (`time -l`) / peak footprint | 21.4 MB / 19.8 MB | 8.2 MB / 6.7 MB |
| engine construction | 97 ms | 17 ms |
| rule turn p50 / p99 (5000 in one process) | 20 / 46 µs | 14 / 15 µs |
| reflector turn p50 / p99 | 700 µs / 1.2 ms | 640 / 660 µs |
| stripped binary | 1 401 152 B | 1 367 600 B |

Medians of 11 runs; max RSS varies by ±0.4 MB from run to run. Reproduce
with `examples/footprint.rs` in `talos-skills` (it prints the resident set
after each stage and, given a count, p50/p99 per turn kind) and with
`printf 'hola\nhello\ncreo que el cine es aburrido\ni think movies are boring\n' | /usr/bin/time -l target/release/talos chat`.

**Cause.** The 433 patterns (165 of the skills, 223 reflector rules, 45
`unless`/distress) were compiled at construction, each twice (a
`RegexSet` per list and a `Regex` per pattern), with their Unicode classes.
A Unicode `\w` is a UTF-8 automaton of ~320 NFA states (18 KB), and
`(?:\w+ ){0,3}` repeats it four times: the largest reflector patterns were
140 KB each, the lists 3.8 MB of NFA in all, and the Thompson compiler
peaks at several times its output (one `RegexSet` of 85 patterns: 6 MB
peak for 1.1 MB kept), which the allocator keeps. The engine held 8.6 MB of
heap after construction, 1.7 million allocations; every `Regex` used then
kept its own matching cache besides.

**What was done** (`talos-core/src/patterns.rs`; nothing a pattern matches
changed, and golden transcripts, replies and `talos explain` traces are
identical over 1 658 inputs):

- **A narrow form of each pattern.** In the match form, accents are gone
  and every space is ASCII, so almost no message holds a word, digit or
  space character beyond ASCII (`¿`, `¡`, `€` are none of those). On such
  a text `\w`, `\d`, `\s` and their negations mean exactly their ASCII
  classes, so each pattern also gets a narrow form with them replaced
  (3.8 MB of NFA → 1.4 MB). A text with such a character (`ß`, `µ`, Greek,
  CJK) is matched with the pattern as written, built the first time one
  arrives. A test checks both forms agree on every golden and example
  line.
- **Built when first needed.** Construction only parses (and so
  validates) the patterns; a list's automata are built on its first
  search, so `ask "hola"` builds the Spanish rules and the distress check
  only.
- **Nothing twice, no kept caches.** A list is matched by multi-pattern
  automata of eight patterns each (small builds keep the transient peak
  low: 8 beat 1, 4, 16, 32 and one per list); a pattern's own automaton,
  for its captures, is built only when it is read. They are the `PikeVM`s
  `regex` would run, configured as it configures them, used through
  `regex-automata` directly (already in the tree under `regex`) so that a
  search's scratch space is freed after it instead of kept per automaton
  (−1.6 MB). Like `regex`, a search is skipped when the text is shorter
  than any match or longer than an anchored one.
- The reflector drops its person tables as written once compiled.

**Tried and rejected.** `regex-lite`: compiles everything in 5 ms, but its
classes are ASCII-only (it would change what `\w` matches), it has no set
(433 `is_match` per turn), and its per-regex caches took the process to
12 MB. Disabling Unicode outright: non-ASCII literals in classes (`[¿¡]`)
do not parse, and non-ASCII letters would stop matching. Size and DFA
limits: no DFA engine is compiled in (no `perf` features), and the NFA
limit only rejects. Factoring common prefixes out of alternations: +2 %
states. Building eagerly at construction, or reusing one compiler per
list: no gain or worse.

**Left.** The automata themselves are ~1.7 MB of heap (a Thompson NFA has
about one state per pattern byte); the rest of the process is the binary,
TOML parsing, the reflector's person tables (~0.45 MB resident) and
allocator fragmentation (on macOS about 45 % of the malloc zone is free
space after construction). Max RSS sits around 8.2 MB, peak footprint
6.7 MB.

`rules.rs` has a guard test: compiling rules builds nothing, a turn builds
only its language's narrow automata, and only a wide text builds the
written form.

## Server

`talos serve` (docs/API.md §2, [ADR 0017](adr/0017-http-stack.md)) is in
`crates/talos/src/server/`: `http.rs` reads and writes HTTP/1.1, `api.rs`
is the routes and the OpenAI shapes, `identity.rs` maps a request to a
conversation. It sits behind the `talos` crate's `server` feature, on by
default and with no dependency of its own. What `ask`, `chat` and `serve`
share (the clock, the `talos` object) is in `crates/talos/src/lib.rs`.

```sh
cargo run --release -- serve                    # 127.0.0.1:8099
cargo run --release -- serve --port 0           # any free port; the address is printed
TALOS_TOKEN=s3cret cargo run --release -- serve --bind 0.0.0.0   # off loopback a token is required
cargo build --release --no-default-features     # a binary without the server
cargo test -p talos --test server               # the end-to-end tests
```

```sh
curl -s http://127.0.0.1:8099/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model": "talos", "messages": [{"role": "user", "content": "qué hora es"}]}'
```

answers (one line, shown here broken):

```json
{"choices":[{"finish_reason":"stop","index":0,"logprobs":null,
  "message":{"content":"Son las 12:02.","role":"assistant"}}],
 "created":1795604532,"id":"talos-0000000000000001","model":"talos",
 "object":"chat.completion",
 "talos":{"confidence":1.0,"elapsed_us":10094,"generated":false,"intent":"clock.now",
  "language":"es","pending":false,"reason":null,"slots":{},"status":"answered"},
 "usage":{"completion_tokens":0,"prompt_tokens":0,"total_tokens":0}}
```

with `X-Talos-Status: answered`. Add `"stream": true` for server-sent
events (`curl -N`), `"metadata": {"conversation": "k"}` or `"user": "k"` to
name the conversation; without either, sending the history back as OpenAI
clients do continues it (the transcript fingerprint).

Tests start servers in-process on an ephemeral port with
`talos::server::{Server, Settings}`, a fixed `Settings::clock` and
`stopwatch = false` (so `elapsed_us` is 0), and speak HTTP to them with a
client written in the test. That is the determinism path: the CLI always
uses the system clock. One test runs the binary itself.

**Measurements** (2026-11-25, Apple-silicon Mac, macOS, release build,
loopback, three runs each):

| | |
|---|---|
| stripped binary without / with `server` | 1 400 864 B / 1 468 192 B (+66 KiB) |
| crates in the binary's tree | 23 either way (20 third-party) |
| RSS at rest (started, no request) | 5.5–6.3 MB |
| RSS after 1 000 / 11 000 requests of fresh one-message conversations (`hola`) | 7.1–7.6 / 7.3–7.6 MB |
| RSS after 1 000 / 11 000 mixed requests (rule and reflector, both languages) | 9.0–9.4 / 9.0–9.4 MB |
| server overhead per request, p50 / p99 | 64 / 122–152 µs |
| round trip, rule turn / reflector turn, p50 | 140 / 730 µs |

How: the binaries are `cargo build --release -p talos` with and without
`--no-default-features`, sized with `ls -l`. RSS is `ps -o rss` of a running
`talos serve --port 0`, read one second after start and after each batch.
Requests go over one keep-alive connection, one at a time:
`spikes/http-stack/measure.sh` (its `load` client, the same body each time)
and `cargo run --release -p talos --example serve_load -- ADDR N`, which
alternates two rule and two reflector messages and reports the round trip,
the engine's own `elapsed_us`, and their difference, the **overhead**: what
the server adds (reading and parsing the request and the JSON, identity,
writing the response, the loopback trip). Every request in both is a fresh
transcript, so the dialog-state store fills to its 1 024 conversations and
evicts; memory stops growing there. `spikes/http-stack/` also holds the
`tiny_http` and `axum` servers the stack was chosen against (ADR 0017).

## Golden transcripts

Golden files live in `golden/<skill>/<name>.txt` (cross-cutting ones in
`golden/engine/`) and are run by `cargo test` through
`crates/talos-skills/tests/golden.rs`. The format mirrors the exchange format
of [spec/README.md](spec/README.md), so spec examples transcribe
mechanically:

```text
# A comment.
Given: now = 2027-03-14 18:42 Europe/Madrid
Given: draw = 3, 5

[es] U: 2d6
     T: 3 y 5: total 8.                           → answered · random.dice · count=2 sides=6

[es] U: hola
     T: ¡Hola!                                    → answered · smalltalk.greet
[es] U: hola
     T: ¡Hola, buenas!                            → answered · smalltalk.greet
```

- **Turns.** `[es] U: <text>` then `T: <reply>`. The tag is the expected
  language of the turn (asserted). A reply of several lines continues on
  indented lines; the outcome then goes on a line of its own starting with
  `→`. Lines after the outcome, up to the next blank line, turn or `Given:`,
  are notes (the spec's `(…)` explanations) and are ignored. `#` starts a
  comment line.
- **Conversations.** Turns with no blank line between them are one
  conversation; a blank line starts a new one (fresh state, turn 1). All
  conversations use the id `golden`, so variants are reproducible.
- **Outcome line.** `→ status · intent · slot=value slot=value` when an
  intent was recognised, `→ status · reason` when none was. Slots are in
  name order, values canonical: numbers with a decimal point and no
  grouping (`1500`, `1.5`), dates in ISO (`2027-04-03`), lists joined by
  `|` (`options=té|café`). Internal slots are left out. A turn that asked
  back is `→ asks · intent · slot`, and the turn that answers it follows
  with no blank line between them (the same conversation); a `Given:` line
  between turns (a new `now`, a `draw`) does not end a conversation. Reply
  and outcome are compared byte for byte.
- **`Given:` lines** last until changed, across conversations, except
  `draw`, which applies to the next turn only:

  | Key | Values | Default |
  |---|---|---|
  | `now` | `YYYY-MM-DD HH:MM[:SS] ZONE`; ZONE is `UTC`, an offset (`+01:00`, `UTC-3`) or one of `Europe/Madrid`, `Europe/London`, `Europe/Paris`, `Europe/Berlin`, `Europe/Rome`, `Europe/Lisbon`, `Atlantic/Canary`, `America/New_York`, `America/Chicago`, `America/Los_Angeles`, `Asia/Tokyo` | `2027-03-14 18:42 Europe/Madrid` |
  | `seed` | the configured seed | `0` |
  | `default language` | `es`, `en` | `es` |
  | `hint` | `es`, `en`, `none`: the caller's language hint | `none` |
  | `place` | a place name or zone, or `none`: the caller's place | `none` |
  | `draw` | comma-separated draws in the spec's natural terms: numbers (`4`, `-2`, `1.73`), `heads`/`tails`, `option N` (1-based), `entry "fragment"` | — |
  | `slots` | `all` (include internal slots), `public` | `public` |

  Anything else after `Given:` is an error, so a spec line such as "Given:
  a conversation that has been in Spanish" must be expressed with turns.
  A turn with a `draw` fails unless it draws exactly what was scripted.
- **Failures** print the file, line, input, and expected against actual.
  **Regenerate deliberately** with `TALOS_BLESS=1 cargo test -p
  talos-skills --test golden`: every T block is rewritten from the actual
  output (comments, `Given:` lines and notes are kept); review the diff
  before committing.
