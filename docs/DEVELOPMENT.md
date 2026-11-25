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
| Pattern engine | `regex` `RegexSet`, one per language, features `std` + `unicode-perl` only | no `perf` engines: a few dozen anchored patterns do not need them |
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
| Slots | declared per intent (`[[intent.slot]]`: `name`, `type` = `term`/`number`/`date`, `required`, `repeated`, `internal`); named captures `(?P<name>…)` must be declared slots; extractors turn captures into typed `Value`s before the handler runs | `internal` slots (Slice A's `register`, `daypart`) are left out of `Outcome::slots` and of golden outcome lines unless `Given: slots = all`; a handler may `cx.report(…)` a default it applied ("tira un dado" → `count=1 sides=6`) |
| Numbers | exact decimals (`Number`: `i128` units and a scale), never floats; `Number::format(lang)` writes them by the language's conventions | the separator rules of conversation.md 2.1 are tested as a table |
| Dates | `civil::Date` in-tree (Hinnant's algorithms), no zone rules | the zone database is still Slice C's decision; the golden harness knows a handful of zones the spec uses, with their DST rules, until then |
| Random source | `SplitMix64` in-tree behind `RandomSource`; each turn is seeded from FNV-1a of (configured seed, conversation, turn number, normalised text), lazily, so a turn that draws nothing seeds nothing | handlers draw through `Random` in natural terms (`int`, `scaled`, `coin`, `pick`, `entry`), which is also what golden files script with `Given: draw = …` |
| Conversation state | engine-owned map, one `Mutex` per conversation plus a short-held map lock; at most `Config::max_conversations` (1024), idle expiry `Config::idle_expiry_seconds` (30 min) measured with the injected `now` | holds the turn counter, the language and how firmly it is held, the recent topic (last intent), and an always-empty expectation slot for v0.2 |
| Language stickiness | a settled conversation switches only on at least two marker words of the other language and more than of its own; a conversation that only defaulted takes the first evidence; a hint settles it | ADR 0015's classifier step waits for v0.2 |
| Greeting prefixes | an intent may be `courtesy = true`; when the whole message matches nothing, a courtesy of up to four words at its start or end is dropped if the rest is recognised | basics.md, "Greeting prefixes"; the reply does not add a greeting of its own |
| Reasons | `Reason` is an open set of names (`empty`, `catch-all`, `cannot`, `cannot-ask`, `failed`; the reflector adds its shapes) | outcome lines show the reason when no intent was recognised |
| Local time in the CLI | the binary runs `date +%z` before each turn | the smallest means with no `unsafe` and no crate; honours `TZ`; where `date` is missing the offset is unknown and time-of-day answers stay neutral. The date/time crate is left to Slice C |
| JSON | `serde_json` (std only) in the binary for `--json` | +17 KB on the stripped binary, measured; the server will need it anyway |
| Validator | `talos_core::check`, run by `talos skills check` and by a test in `talos-skills` that requires zero findings | see the list of checks in `check.rs` |

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
  `|` (`options=té|café`). Internal slots are left out. Reply and outcome
  are compared byte for byte.
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
