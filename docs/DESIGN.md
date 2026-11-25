# Design

This is the architecture of Talos as intended. It describes the target; the
engine and the first skills exist (see [ROADMAP.md](ROADMAP.md)), and where a
section says what is built today, the code is the authority. Each significant
choice links to the decision record that justifies it.

## 1. Goals

1. **Answer without a language model.** Everything Talos says comes from a
   skill, the knowledge store, or a template. ([ADR 0012](adr/0012-no-language-model.md))
2. **Be deterministic.** Same text, same state, same clock: same answer. The
   only exception is an outcome explicitly marked `generated`, which can come
   solely from a user-installed external skill.
3. **Stay small.** See the budget in §8. Every dependency has to earn its
   weight.
4. **Speak Spanish and English as equals.** ([ADR 0011](adr/0011-bilingual-by-construction.md))
5. **Run alone.** No host application required; no network required for the
   core.
6. **Know when it does not know.** Every outcome states whether the message
   was handled.
7. **Be teachable and explainable.** What Talos knows has a source, and a
   derived answer can show the path that produced it.

### Non-goals

- Generating novel prose, summarising arbitrary text, planning.
- Personality, emotion or mood modelling; proactive messages.
- Multi-user accounts and authorisation beyond a single shared token.
  Preferences and taught knowledge are one set per installation.
- Embeddings and vector search in the first major version.
- A graphical interface.

## 2. Shape of the system

```
                    ┌──────────────────────────────────────────┐
  talos chat  ───┐  │                 Engine                   │
  talos ask   ───┼─▶│  Turn ─▶ pipeline ─▶ Outcome             │
  talos serve ───┘  │                                          │
                    │  NLU model   Skill registry   Dialog map │
                    └──────┬──────────────┬─────────────┬──────┘
                           │              │             │
                     model.bin      skill manifests   SQLite
                    (read-only)    + reflector scripts  ├─ user.db   (read-write)
                                                        └─ packs/*.talospack (read-only)
```

One process, one binary. The **engine** is a library with a single entry
point, `engine.respond(turn) -> outcome`. The three front ends (interactive
chat, one-shot, HTTP server) are thin shells around that call.
([ADR 0010](adr/0010-one-binary-three-front-ends.md))

The engine is synchronous and does no I/O of its own beyond SQLite. It has no
event bus and no layers: a turn moves through a fixed sequence of stages as a
value passed from function to function. ([ADR 0004](adr/0004-explicit-turn-pipeline.md))

### Proposed source layout

```
crates/
  talos-core/        the engine: turn, pipeline, NLU, dialog, rendering
  talos-knowledge/   the store: schema, queries, reasoning, packs
  talos-skills/      built-in skills
  talos/             the binary: CLI and HTTP server
skills/              skill manifests (TOML), one directory per skill
reflector/           fallback scripts, one per language
evals/               held-out evaluation batteries
tools/
  train/             offline classifier trainer (Python, dev-time only)
  import/            pack builders (WordNet family first)
docs/
```

`talos-core` must not depend on an async runtime, an HTTP stack, or the
network. That keeps the engine embeddable and its tests fast.

## 3. The turn

```rust
struct Turn {
    text: String,            // exactly as received
    lang: Option<Lang>,      // caller's hint; detected if absent
    conversation: ConvId,    // which dialog this belongs to
    now: Timestamp,          // injected, never read from the clock mid-turn
    place: Option<Place>,    // caller-provided: a place name or an IANA zone
    trace: bool,             // ask for the stage-by-stage trace in the outcome
}

struct Outcome {
    reply: String,
    status: Status,          // Answered | Deflected | Unhandled
    lang: Lang,
    intent: Option<IntentId>, // kept even when the status is Unhandled
    slots: Slots,            // what was extracted: typed values by name
    confidence: f32,         // of the intent recognition, not of any fact
    reason: Option<Reason>,  // why a turn is Unhandled or which shape Deflected
    pending: bool,           // Talos asked back and expects the next message
    generated: bool,         // true only if an external skill used a model
    trace: Option<Trace>,    // stage-by-stage decisions, if the turn asked
}

enum Value {                 // a slot value, typed by the manifest
    Text(String), Number(Number), Date { date: Date, year_given: bool },
    List { items: Vec<String>, uncertain: bool },
    Invalid { raw: String, problem: Problem },   // could not be read
}
```

`Reason` is an open set of lower-case, hyphenated names, not an enum: the
engine's own (`empty`, `catch-all`, `cannot`, `cannot-ask`, `failed`) and the
sentence shapes the reflector adds. `Number` is an exact decimal, so a number
loses nothing between extractor and reply.

Two rules follow from this shape:

- **Context travels in the value.** Language and conversation are fields of
  the turn. No stage can lose them, because there is no other way to reach a
  stage.
- **Time is an input.** `now` is injected so that a turn can be replayed and
  tested. It is a Unix instant plus, when the caller knows it, the local
  offset from UTC; without the offset, answers that depend on the wall clock
  stay time-neutral.
- **The turn number is the engine's.** The turn carries no counter: the
  engine keeps each conversation's turn count, language and recent topic
  (§7) and hands them to skills through the context.

`Status` has three values on purpose ([ADR 0009](adr/0009-reflector-fallback.md)):

| Status | Meaning | A host should… |
|---|---|---|
| `Answered` | Talos gave a complete and correct response, including a correct "no" or "that cannot be done" | show it |
| `Deflected` | The reflector matched a real pattern | show it, or pass the turn on |
| `Unhandled` | Talos could not provide what was asked: nothing recognised it, or it was recognised and is beyond what Talos knows or does | pass the turn on |

## 4. The pipeline

```
text
 │
 ▼
0 normalise ──▶ 1 language ──▶ 2 pending dialog ──▶ 3 rules ──▶ 4 classifier
                                     │                 │             │
                                     └────────┬────────┴─────────────┘
                                              ▼
                                        5 dispatch (skill)
                                              │  Answer / Ask / Decline
                                              ▼
                                         6 render ──▶ Outcome
                                              ▲
                                        7 reflector (if nothing answered)
```

**0. Normalise.** Unicode NFKC, case folding, whitespace, typographic variants
(superscripts, `×`, curly quotes). Three forms are kept, aligned character by
character: the original; the normalised text, which keeps accents because they
are evidence of language; and the *match form*, the normalised text with
diacritics removed from Latin letters (`¿Qué día es MAÑANA?` becomes
`¿que dia es manana?`). Rules and extractors match the match form, so patterns
are written lower-case and unaccented; a capture is read back from the
original, in the user's own spelling. Skills that need the original
(translation, teaching) read it.

**1. Language.** A caller's hint wins. Otherwise words and characters that
belong to only one of the two languages decide; failing that, a tiny two-class
linear classifier; failing that, the conversation keeps its language. The
result is sticky per conversation with hysteresis, so a single ambiguous word
("ok", "chess") does not flip it. ([ADR 0015](adr/0015-language-detection.md))

**1b. Safety check.** A message that reads as someone in distress is answered
here, before anything else can claim it: a fixed reply that points to
emergency help, with numbers taken from configuration for the user's place.
The status is `Unhandled`, so a host with something better can step in. This
runs ahead of the pending dialog so that an open yes/no can never swallow it.

**2. Pending dialog.** If the conversation has an open *expectation* (a slot
to fill, a yes/no, a choice between senses), this message is offered to it
first. Expectations are typed, expire after a few turns, and can be cancelled
by the user. An expectation that does not recognise the message releases it to
the rest of the pipeline instead of swallowing it. This stage also answers the
small dialog moves itself — an acknowledgement ("ok", "vale"), a cancellation,
a hesitation — as `Answered` with no intent, since they belong to no skill.

**3. Rules.** Patterns declared in skill manifests, compiled once into a
per-language set. Ordered by explicit priority, then by specificity. A rule
match is certain: it yields an intent and slots with confidence 1.0. An intent
marked `courtesy` (a greeting, a thanks) may also open or close a message that
carries a request: when no rule matches the whole message, a courtesy of up to
four words at either end is dropped if what remains is recognised ("hola, qué
hora es" is answered as the clock question). A small
table of user corrections ("when I say X, I mean Y") is consulted here too, so
teaching Talos a phrase needs no retraining.

**4. Classifier.** Character n-gram features into a linear model trained
offline from the same manifests. It returns a ranked list; the top candidate
is accepted only above an absolute threshold *and* with a margin over the
runner-up. Out-of-scope examples are a real class. Slots are then extracted by
typed extractors (today: term, number, date; planned: duration, place,
language).
([ADR 0008](adr/0008-intent-classification.md))

**5. Dispatch.** The intent's skill receives the turn, the slots and a context
handle (knowledge store, preferences, dialog state). It returns one of:

- `Answer(message)` — facts to render.
- `Ask(expectation, message)` — a question back; opens an expectation.
- `Unable(message)` — recognised, but Talos cannot provide it (an unknown
  word, an unsupported unit). Rendered as an honest reply; status `Unhandled`
  with the intent kept.
- `Decline` — "not mine after all"; the next candidate is tried.
- `Fail(error)` — rendered as an honest error.

Skills return *structured messages* (a template id and variables), never
finished prose.

**6. Render.** The message is looked up by `(skill, message id, language)` and
filled in. Where a template has variants, the choice is a pure function of
conversation and turn number: varied to the user, reproducible in a test.

**7. Reflector.** If nothing answered, an ELIZA-style script produces a
reflection. A match on a real sentence shape is `Deflected`; its guards
(factual questions, requests for text, noise, other languages, long text),
the requests it recognises but cannot meet (advice, commands) and the bare
catch-all are `Unhandled`, with the shape or reason in `Outcome::reason`.
([ADR 0009](adr/0009-reflector-fallback.md), docs/spec/reflector.md)

### Trace

Every stage records what it decided and why. `talos explain "…"` and the
`trace` option on the API return it. Today the stages that report are
`normalise` (the match form), `language`, `safety` (distress, and the region
whose numbers were given), `rules` (candidates with priority, or the courtesy
dropped), `slots`, `dispatch`, `render` (reply and variant), `reflector`
(shape or guard, echo or why not, variant), `fallback` (an empty message),
`random` (draws) and `state` (turn number, language, recent topic); the
pending dialog and classifier join as they are built (v0.2 and later). This is the main debugging tool and the
reason the pipeline is a list of functions and not a graph of subscribers.

## 5. Knowledge

One SQLite database for what the user teaches, plus read-only *packs* attached
at start-up. Concepts are language-neutral; words in each language point at
them through senses. Relations are claims with confidence, provenance and
validity. Transitive reasoning runs inside SQLite as bounded recursive
queries. Details in [KNOWLEDGE.md](KNOWLEDGE.md);
decisions in [ADR 0005](adr/0005-sqlite-knowledge-store.md) and
[ADR 0006](adr/0006-knowledge-packs.md).

## 6. Skills

A skill is a manifest (intents, examples, patterns, slots, replies in both
languages) plus a handler. Built-in handlers are compiled in; external ones
live outside the process and are reached over a protocol still to be chosen. Details in
[SKILLS.md](SKILLS.md); decisions in [ADR 0007](adr/0007-declarative-skill-manifests.md)
and [ADR 0013](adr/0013-external-skills.md).

## 7. State and concurrency

| State | Lives in | Survives restart |
|---|---|---|
| NLU model, compiled rules, skill registry | memory, immutable after start | rebuilt |
| Dialog state (turn count, language, recent topic; expectations from v0.2) | memory, per conversation | no |
| Preferences, taught claims, corrections | `user.db` | yes |
| Packs | `*.talospack`, read-only | yes |
| Turn log | off by default; batched when on | optional |

The engine is shareable across threads: the immutable parts are shared, dialog
state sits in a map with one lock per conversation, and SQLite connections come
from a small pool (WAL mode: many readers, one writer). Turns of the same
conversation are serialised; turns of different conversations are independent.

Dialog state is bounded so that memory cannot grow without limit: at most
`max_conversations` are held (default 1024; the least recently seen goes
first, ties broken by id), and a conversation idle for longer than
`idle_expiry_seconds` (default 30 minutes) starts afresh. Idleness is measured
with the turns' injected `now`, never the system clock. Both are fields of the
engine's `Config`, with the random seed (default 0).

Nothing is written to disk on an ordinary turn. Writes happen only when the
user teaches, corrects, or sets a preference.

## 8. Budgets

These are targets to be measured, not measurements. Each roadmap milestone
carries the subset it must meet.

| Metric | Target | Measured on |
|---|---|---|
| Resident memory at rest, no packs | ≤ 10 MB | Raspberry Pi (ARM64) |
| Resident memory at rest, one large pack | ≤ 20 MB | Raspberry Pi |
| Turn latency, offline skill, p50 / p99 | ≤ 2 ms / ≤ 10 ms | Raspberry Pi |
| Cold start to first answer | ≤ 100 ms | Raspberry Pi |
| Binary size, stripped | ≤ 15 MB | release build |
| Classifier model on disk | ≤ 2 MB | — |
| Disk writes per ordinary turn | 0 | — |

## 9. Technology

Rust, stable toolchain ([ADR 0003](adr/0003-rust.md)). Status of each
candidate dependency:

| Need | Candidate | Status |
|---|---|---|
| SQLite | `rusqlite` (bundled, FTS5) | undecided; with the knowledge store |
| HTTP server | `axum` on a single-threaded `tokio` runtime | undecided; behind a `server` feature |
| CLI | `clap` | not used: arguments are parsed by hand in the binary |
| Patterns | `regex` | in use (rules; `unicode-perl` only); `aho-corasick` undecided |
| Serialisation | `serde`, `toml`, `serde_json` | in use: `serde` + `toml` for manifests, `serde_json` in the binary only |
| Normalisation | `unicode-normalization` | in use (NFKC and diacritic folding) |
| Dates and zones | `chrono`, `chrono-tz` | undecided: `Timestamp` and `Date` are own code; zones come with `timezones` |
| Outbound HTTP | `ureq` | undecided; behind a `net` feature |
| Arithmetic | own parser, or `fasteval` | undecided; exact decimals are own code (`Number`) |
| Chess rules | permissively licensed crate + own search | undecided; no GPL in the binary |

Policy: prefer no dependency, then a small one, then a feature-gated one.
Licences must be permissive (MIT, Apache-2.0, BSD, public domain).

## 10. Configuration and files

A single `talos.toml`, overridable by environment variables and flags. Among
other things it holds the default language, the default place and time zone,
the seed of the random source (fixed in tests, set at start-up otherwise),
the identity line Talos gives when asked who made it, and the emergency
numbers to show for each place.

```
$XDG_CONFIG_HOME/talos/talos.toml
$XDG_DATA_HOME/talos/
    user.db
    packs/
    skills/          user-installed manifests and external skills
```

Built-in manifests, reflector scripts and the classifier model are embedded in
the binary, so a bare executable works with no installation.

## 11. Security

- The server binds to `127.0.0.1` by default; a bearer token is required to
  bind anywhere else.
- Skills declare capabilities (`network`, `filesystem`, `exec`); configuration
  grants them. Built-in skills that need the network are off until enabled.
- External skills are an allow-list in configuration, never discovered and run
  implicitly.
- No skill is ever dispatched to a side-effecting action on classifier
  confidence alone: actions with effects require a rule match or an explicit
  confirmation. Among built-in intents the side-effecting ones are those that
  write: `teach.assert`, `teach.forget`, `teach.phrase`, `preferences.set`
  and `preferences.forget`.

## 12. Testing and evaluation

- **Unit tests** per stage; the pipeline is pure enough to test without
  fixtures.
- **Golden transcripts**: conversations replayed with a fixed clock, compared
  byte for byte. Determinism makes this cheap.
- **Evaluation batteries** in `evals/`: held-out phrases per language with a
  rejection set, never derivable from the manifests. `talos eval` reports
  accuracy, false accepts and fallback rate; CI enforces floors.
- **Benchmarks**: latency and allocation per stage; memory measured on the
  target hardware, not inferred from a laptop.

## 13. Open questions

1. **External skill transport.** Talos's own JSON protocol, the Model
   Context Protocol, or both. MCP is the leading candidate if skills are to be
   shared with a host such as Tomos. Decided in v0.5.
   ([ADR 0013](adr/0013-external-skills.md))
2. **Pack attachment vs. merge.** Attaching packs keeps them shareable and
   read-only; it needs a spike to confirm query cost across several attached
   databases. ([ADR 0006](adr/0006-knowledge-packs.md))
3. **Conversation identity for stateless clients.** The transcript-fingerprint
   scheme in [API.md](API.md) needs to be validated against real OpenAI-style
   clients.
4. **Timers and reminders.** They need a way to speak first. Out of scope until
   there is a design for interruption.
5. **Search backend.** Which sources the `search` skill uses by default, and
   whether a host-provided search endpoint is the primary path.
