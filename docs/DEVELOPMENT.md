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
cargo clippy --workspace --all-targets --all-features -- -D warnings   # lints
cargo test --workspace --all-features                    # tests, with the knowledge store
cargo test --workspace --no-default-features             # tests, without it (user.toml, no packs)
cargo deny check                                         # licences, advisories, bans
cargo build --release                                    # small, stripped binary
TALOS_BLESS=1 cargo test -p talos-skills --test golden   # regenerate golden files
cargo run --release -p talos-train                       # retrain the classifiers (models/)
cargo run --release -p talos-train -- --check            # fail if models/ is not what training gives
cargo run --release -p talos-train -- --cv 5             # cross-validate on the training corpus
cargo run --release -- eval                              # score the batteries, rules only and with the classifier
cargo run -p talos-pack                                  # build the seed pack (knowledge/seed.toml) into target/packs/core.talospack
cargo run -p talos -- packs check target/packs/core.talospack   # verify a pack whole
cargo run --release -p talos-pack -- wordnet DIR         # the vocabulary pack from the wordnets in DIR ("The vocabulary pack")
```

Lints are shared through `[workspace.lints]` in `Cargo.toml`
(`unsafe_code` is forbidden). `spikes/` is excluded from the workspace;
`tools/` holds development-time workspace members (the trainer, the pack
builder), never linked into the binary.

Retrain after changing any manifest's `[intent.examples]` or
`reflector/out-of-scope.toml`: the engine refuses models trained on another
corpus (rules only) and `the_embedded_models_match_the_manifests` fails until
`models/` is regenerated. The trainer and the models are described in
[MODEL.md](MODEL.md).

## CI

`.github/workflows/ci.yml` runs on every push and pull request:

| Job | Runs |
|---|---|
| `check` | the fmt, clippy and test commands above, and `talos-train --check` |
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

The musl target gives a static binary that runs on any ARM64 Linux. Besides
a linker, the bundled SQLite of the `knowledge` feature needs a C compiler
for the target; zig provides both (`cargo zigbuild` sets `CC`).

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
| Dates | `civil::Date` in-tree (Hinnant's algorithms), no zone rules | zone rules came with `timezones` (v0.2 Slice C, below); the golden harness still knows only a handful of zones for `Given: now` |
| Random source | `SplitMix64` in-tree behind `RandomSource`; each turn is seeded from FNV-1a of (configured seed, conversation, turn number, normalised text), lazily, so a turn that draws nothing seeds nothing | handlers draw through `Random` in natural terms (`int`, `scaled`, `coin`, `pick`, `entry`), which is also what golden files script with `Given: draw = …` |
| Conversation state | engine-owned map, one `Mutex` per conversation plus a short-held map lock; at most `Config::max_conversations` (1024), idle expiry `Config::idle_expiry_seconds` (30 min) measured with the injected `now` | holds the turn counter, the language and how firmly it is held, the recent topic (last intent), and the open question (at most one, with the request's slots; it expires three turns or five minutes after it was asked) |
| Language stickiness | a settled conversation switches only on at least two marker words of the other language and more than of its own; a conversation that only defaulted takes the first evidence; a hint settles it | ADR 0015's classifier step waits for v0.2 |
| Greeting prefixes | an intent may be `courtesy = true`; when the whole message matches nothing, a courtesy of up to four words at its start or end is dropped if the rest is recognised | basics.md, "Greeting prefixes"; the reply does not add a greeting of its own |
| Reasons | `Reason` is an open set of names (`empty`, `catch-all`, `cannot`, `failed`, `acknowledgement`, `cancelled`, `hedge`, `nothing-pending`, `gave-up`, `unknown-command`, `command-argument`; the reflector adds its shapes) | outcome lines show the reason when no intent was recognised |
| Local time in the CLI | the binary runs `date +%z` before each turn | the smallest means with no `unsafe` and no crate; honours `TZ`; where `date` is missing the offset is unknown and time-of-day answers stay neutral. The date/time crate is left to Slice C |
| JSON | `serde_json` (std only) in the binary for `--json` | +17 KB on the stripped binary, measured; the server will need it anyway |
| Validator | `talos_core::check`, run by `talos skills check` and by a test in `talos-skills` that requires zero findings | see the list of checks in `check.rs` |

## Decisions taken in v0.2 Slice A (the classifier)

Details in [MODEL.md](MODEL.md).

| Question | Taken | Note |
|---|---|---|
| Trainer | a Rust workspace member, `tools/train` (`talos-train`), not Python | no second toolchain; it calls the engine's own feature extraction and file writer, so training and inference cannot drift; dev-time only, never linked into `talos` (ADR 0003 allows Python, does not require it) |
| Optimiser | own SGD for softmax regression with lazy L2, class weights `(mean / size)^0.5`, two fits (choose features, then refit on them), prune at 2 % of the largest weight, signed-byte weights | no ML crate; `exp`/`ln` are own code so files are byte-identical across machines |
| Model in the binary | `include_bytes!` from `models/` via `talos-skills/build.rs`, read in place | the image is paged in on demand like an mmap, without `unsafe` or an installed file; a rule turn never touches it. Without `models/` at build time the engine is rules only |
| Wrong model | refused when its corpus hash is not the hash of the manifests and `reflector/out-of-scope.toml`; rules only, the trace says why; a test fails | the out-of-scope file is embedded for the hash |
| Out-of-scope | a class trained from `reflector/out-of-scope.toml` (both languages): generation, open questions, future skills, statements, near misses, the reflector's shapes, acknowledgements, noise | the classifier's "none of the above" |
| Acceptance | threshold 0.6 (0.85 for a one-word message), margin 0.3 over the runner-up, never an intent with side effects (`classifier::SIDE_EFFECTS`) | chosen by 5-fold cross-validation on the training corpus; the batteries were only ever run whole |
| Slots of a guess | no captures; the fixed slots the intent's rules set most often on its examples, recorded per class by the trainer | `identity` needs a `topic` to answer; a handler that needs a capture gives its usage hint; one that fails sends the turn to the reflector |
| Language, step 3 | the same machinery, two classes, no biases; asked only when markers decide nothing and the conversation has not settled; decides at ≥ 0.95 and only with a word of 3+ letters; holds like a default | "bye", "thx", "vale" now get their language; "hey", "ok", "d20", "2+2" keep the default, as the golden files require |
| `talos eval` | two engines, rules only and with the classifier, side by side; built-skill accuracy; conversation catch-all rate | counts only, never phrases |

## Decisions taken in v0.2 (preferences, corrections, guessed slots)

| Question | Taken | Note |
|---|---|---|
| Where the user's data lives | `user.toml` in the data directory: `$TALOS_DATA_DIR`, else `$XDG_DATA_HOME/talos`, else `$HOME/.local/share/talos`; `TALOS_DATA_DIR=` (empty) keeps nothing between runs | the simplest durable form until `user.db` (v0.3, DESIGN.md §7): a versioned TOML file (`version = 1`) whose fields are the future tables' columns (`[preferences]` name, place, units; `[[correction]]` phrase, intent, slots), read with the `toml` the engine already has and written by hand, so no new dependency |
| How it is written | the whole file, to `user.toml.tmp`, flushed (`sync_all`) and renamed over the old one; only on a turn whose changes differ from what is stored | a crash leaves the old file or the new one; an ordinary turn, or setting a value to what it already is, writes nothing (DESIGN.md §8, "Disk writes per ordinary turn: 0") |
| Who "the user" is | the installation: one set of preferences and corrections shared by `talos ask`, `chat`, `explain` and `serve`, whatever conversation or API `user` a turn comes from | docs/spec/decisions.md ruling 10; `talos eval` and `skills` never read it, so a battery's score does not depend on who ran it |
| A file that cannot be read | a warning on stderr; the engine keeps changes in memory for that run and never overwrites the file | "degrade cheaply" (DESIGN.md §1) |
| Hermetic tests | the engine's default store is `talos_core::MemoryStore`: nothing is read or written unless a front end calls `Engine::use_user_store`; golden files set what is stored with `Given: stored …` and `Given: phrase …`, reset at each conversation; the server test that spawns the binary points `TALOS_DATA_DIR` at a temporary directory; `FileStore`'s own test writes only under the system temporary directory | no test reads or writes the real home directory |
| How a skill changes a preference | `cx.remember(PreferenceChange::…)`; the engine applies the changes after the handler answers (`SkillResult::Answer`), and drops any made on a classifier's guess without a `confirm` slot answered yes (the turn fails instead) | handlers stay pure: no handler touches a store (SKILL_AUTHORING.md §3.3) |
| Side-effecting guesses | `preferences.set` and `preferences.forget` stay in `classifier::SIDE_EFFECTS`: never accepted; when one passes the threshold and the margin it is offered to its handler with `cx.guessed()` set, which may only decline or ask a yes/no that restates the change | DESIGN.md §11: "a rule match or a confirmation" |
| The user-correction table | phrase (the match form without end punctuation) → intent + slots as text, looked up before every pattern at the rule stage (also while a question is open); its slots are read by their extractors in either language; never an intent that deletes (`preferences.forget`, `teach.forget`, `teach.phrase`) | ADR 0008. The spec gives it no user-facing phrasing before `teach.phrase` (knowledge.md, v0.3): `Engine::set_user_data` and the file are the only ways in for now |
| Slots of a guess | as before, plus: a public slot whose manifest declares `guess` cues (`before`, `after`, `words`) is looked for next to them and read by its extractor; a value found twice and different, or unreadable, gives nothing | conservative: a wrong capture is worse than a question ("lanza tres dados" rolls three; "para mí que…" is not a question about the user) |
| A capture in the other language | a rule of the other language whose capture does not read in the turn's language is read in the rule's ("roll a twenty sided die" in a Spanish conversation) | — |
| `/dado 2d6` | a command may fill an internal slot: `random.dice`'s `notation` takes the rest of the line (`2d6`, `d20`, `3d8+2`, `20` = sides), which the handler reads and reports as the sentence form's slots | ADR 0016: "one free-text argument its handler reads"; the outcome is the sentence form's |

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

### With the classifier (v0.2)

Measured 2026-11-25 on the same Mac, release build, seven runs each:

| | no models in the build | with the models |
|---|---|---|
| stripped binary | 1 617 120 B | 2 244 576 B (+620 KB of models) |
| `talos ask "hola"`: max RSS / peak footprint | 7.1–7.5 / 5.4–5.8 MB | 7.0–7.7 / 5.3–5.9 MB |
| the four-turn chat above: max RSS / peak footprint | 8.6–8.9 / 6.9–7.2 MB | 9.1–9.7 / 6.8–7.5 MB |
| the same plus two turns the classifier answers | — | 9.2–9.8 / 6.9–7.5 MB |
| classifier stage alone, p50 / p99 (`footprint 5000`) | — | 20 / 32 µs |
| turn answered by the classifier, p50 | — | 0.5–0.8 ms (mostly the rules of both languages and the courtesy retries that run first) |

Both columns carry v0.2's larger manifests (about 3 000 examples), which
account for the rise of the no-model build over the figures above. The models
are read in place from the executable's image ([MODEL.md](MODEL.md) §5): a
rule-matched turn never touches them, and a turn that reaches the classifier
(every reflector turn does) brings in clean, file-backed pages of them, up to
their 620 KB. They count in the resident set but not in the peak footprint,
which counts dirty memory, and the system can drop them under pressure. The
CLI target of ≤ 8 MB resident holds for one-shot rule turns; a chat that
reaches the classifier and the reflector exceeds it by about 1.5 MB, of which
the models are under 0.7 MB.

### Closing v0.2: groups built only for texts they could match

Measured 2026-11-25 on the same Mac, release build, seven runs each,
interleaved; "before" is the tree at the end of Slice C, "after" the tree
with the corpus and rules of the v0.2 close (skill patterns 273 → 381,
about 4 400 intent examples).

| max RSS / peak footprint, MB | before | after |
|---|---|---|
| `talos ask "hola"` | 7.8–8.5 / 5.9–6.6 | 7.5–8.1 / 5.5–6.1 |
| the four-turn chat above | 9.9–10.5 / 7.4–8.1 | 9.0–9.5 / 6.4–6.9 |
| the same plus two turns the classifier answers | 10.2–10.7 / 7.7–8.2 | 9.1–9.7 / 6.5–7.1 |
| stripped binary | 2 628 144 B | 2 793 440 B (models 0.78 MB) |

**Where it went.** A heap snapshot (`malloc_history -allBySize` on a
symbolised build of `examples/footprint.rs`) after one turn of each kind
held 4.0 MB in 19 400 allocations, 2.3 MB of them the pattern automata
(1.2 MB the rules of both languages, 0.9 MB the reflector's), and the
malloc zone was 46 % free space left by the transient peaks of building
them. The manifests' examples were kept in memory, ~3 500 strings, but they
were not the cost: parsing them without keeping them (below) moved the
resident set by less than the run-to-run noise. The growth was the rules:
every pattern of a language's list was built on its first search, and the
list had grown by two thirds since v0.1; adding the v0.2 close's rules took
the chat to 11.0–11.4 MB.

**What was done** (nothing a pattern matches changed; a test checks, on
every golden and example line, that the groups' matches are those of each
pattern alone):

- **Groups built only when they could match.** Each pattern records the
  words one of which any match must contain, read from its syntax tree
  (`cuanto\s+es` needs "cuanto"; an alternation needs a word of each
  branch; optional parts and classes need nothing). A group whose every
  pattern needs a word the text lacks is neither built nor searched, so a
  conversation builds only the groups its messages could match. Groups are
  now 4 patterns, not 8: with lazy groups, smaller ones build less for
  patterns that cannot match (2 and 4 were close; 8 and 16 kept more), and
  rule turns got no slower (p50 14–20 µs).
- **Examples digested, not kept.** The engine parses manifests with
  `Manifest::parse_lean` and the out-of-scope corpus with
  `parse_out_of_scope_lean`: each example is hashed as the TOML parser
  hands it over and never allocated; the corpus hash (MODEL.md §5) is built
  from those digests. Only the trainer and `talos skills check` keep the
  text. This halves manifest parsing time; its effect on memory is small.

**Left.** The chat that reaches the classifier and the reflector is still
about 1 MB over the 8 MB target, most of it the binary's touched pages, the
models' clean pages and the reflector's automata and tables.

## Decisions taken in v0.2 Slice C (`timezones`)

| Question | Taken | Note |
|---|---|---|
| Zone data | **no crate**: an in-tree table of 161 IANA zones, each with its POSIX TZ rule (`crates/talos-skills/src/places/zones.rs`), evaluated by ~150 lines of own code (`tz.rs`); never the host's zoneinfo | measured below; deterministic by construction; correct from 2027-03-14 for as long as the rules of IANA tzdata 2026f hold |
| Places | an in-tree gazetteer (`places/table.rs`, read through `crates/talos-skills/src/places.rs`, which `timezones` and `preferences` share): 293 cities and 141 regions (countries, a few US states, island groups), 753 names in the match form, Spanish and English | compiled by hand from public facts (capitals, the largest cities, the places the specification names); no dataset was downloaded |
| Place slots | `term`, read by the skill with its gazetteer and `extract::place` | the skill splits "Lima y Bogotá", tells same-named cities apart and reports the name it used |

### Zone data

**Candidates**, each a standalone program with the release profile above
that prints a zone's offset now, against a 286 128 B baseline with no zone
code; macOS arm64, max RSS and peak footprint the median of five runs
looking up `Asia/Tokyo`:

| | stripped binary | Δ | max RSS / peak footprint | deterministic |
|---|---|---|---|---|
| baseline | 286 128 B | — | 1.77 / 1.26 MB | — |
| `jiff` 0.2.38, `tzdb-bundle-always` (all ~600 zones, compressed) | 683 328 B | +397 KB | 1.93 / 1.26 MB | yes |
| `jiff` 0.2.38, `tzdb-zoneinfo` (the host's files) | 468 896 B | +183 KB | 2.26 / 1.56 MB | no: the answer follows the machine's tzdata |
| `chrono` 0.4 + `chrono-tz` 0.10.4 (all zones, compiled in) | 1 491 952 B | +1 206 KB | 2.85 / 2.29 MB | yes |
| in-tree rules (taken) | — | not measured apart: 7 KB of rule table source and ~150 lines of evaluator; the whole skill is measured below | — | yes |

**The whole skill, in `talos`** (before: the tree at the start of C1; after:
with `timezones`, its manifest, the retrained models and the gazetteer;
max RSS / peak footprint, the range of the medians of three rounds of seven
runs each, before and after measured alternately):

| | before | after |
|---|---|---|
| stripped binary | 2 311 136 B | 2 478 800 B (+164 KB: +30 KB models, +30 KB manifest text, the rest gazetteer, rules and code) |
| `talos ask "hola"` | 7.52–7.77 / 5.70–5.95 MB | 7.60–7.96 / 5.72–6.08 MB |
| `talos ask "qué hora es en Tokio"` | 9.03–9.16 / 6.69–6.82 MB (reflector) | 7.83–8.19 / 5.90–6.26 MB |
| the four-turn chat above | 9.50–9.96 / 7.18–7.64 MB | 9.85–10.21 / 7.46–7.78 MB |
| the same plus two `timezones` turns | 9.57–9.91 / 7.21–7.55 MB | 9.85–10.29 / 7.39–7.83 MB |

The rise at rest (about 0.1–0.2 MB, inside the run-to-run noise) is the 43
new patterns' automata and the larger manifests, not the zone data: a
`timezones` turn costs about 0.2 MB over a greeting.

**Why it is exact.** The IANA database ends every compiled zone with a
POSIX TZ rule for the years after its listed transitions ("the last Sunday
of March at 02:00"). Today's rules of every zone in the table are such a
rule; only one-off changes (a country that skips a change one year, or
changes its law) are not, and those are dated. `generate.py`, beside the
table, writes it from `/usr/share/zoneinfo` (the footers) and writes
`transitions.txt`: every change of offset from 2027 to 2036 as Python's
`zoneinfo` computes it from the full database. A test compares the two:
from 2027-03-14 (Cuba's return to its rules, the latest one-off in tzdata
2026f) to the end of 2036 they agree for all 161 zones. Measured over
2000–2037, the latest disagreements before that are Morocco (permanent UTC+0
from 2026-09-20), British Columbia and Alberta (permanent UTC-7 and UTC-6 from
2026-03-08), Kazakhstan and Paraguay (2024): dates before 2027 may be wrong
for the zones that changed since, which matters only for golden files that
set such a `now`.

**Updating.** When IANA publishes a change to a zone in the table, install
the new tzdata, run `python3 -I crates/talos-skills/src/places/generate.py
2027 2036` from the repository root (it reads the zones from `table.rs`)
and review the diff of `zones.rs`; move the start of the test's span if a
one-off change lands later than 14 March 2027.

## Decisions taken in v0.3 Slice A (the store)

The first part of [plans/v0.3.md](plans/v0.3.md): the SQLite store, `user.db`,
packs, the seed pack and the dictionary queries. The design is
[KNOWLEDGE.md](KNOWLEDGE.md); what is settled here is how it was built.

| Question | Taken | Note |
|---|---|---|
| SQLite binding | `rusqlite` 0.37 with the **bundled** SQLite 3.50, compile options trimmed in `.cargo/config.toml` (`LIBSQLITE3_FLAGS`) | measured below; no system library to depend on (the Pi build is a static musl binary), +620 KB on the binary, memory unchanged within noise. `rusqlite` with no optional feature; it and `libsqlite3-sys` are MIT, SQLite is public domain, their dependencies MIT/Apache-2.0 except `foldhash` (Zlib, through `hashlink`; allowed by `deny.toml`). Both contain `unsafe` (they are the FFI binding); the project's own code has none |
| Feature | `knowledge` on `talos-core` (off by default there), on by default in `talos` (`knowledge = ["talos-core/knowledge"]`) and in `talos-pack` | `--no-default-features` builds the binary without SQLite, byte for byte the size it had before (2 793 440 B): preferences and corrections stay in `user.toml`, `talos packs` says the build has no store. `talos-core` stays synchronous |
| Where the code is | `talos_core::knowledge` (schema, migrations, `user.db`, packs, queries, inflection, sense choice, the pack builder's core); the placeholder crate `talos-knowledge` was removed | the store must be reachable from `Context` and engine tests must build one, which a crate depending on `talos-core` could not give `talos-core` |
| Full-text search | compiled out (no FTS5); `gloss_fts` is not in schema version 1 | the plan's ruling: a dictionary looks words up by key. FTS5 costs 117 KB; a later schema version adds the table by migration and the flag back |
| `user.db` journal | rollback journal (`journal_mode=DELETE`), `synchronous=FULL`, not WAL | reading a WAL database creates `-wal`/`-shm` files, a write on a turn that changes nothing; one process, rare writes |
| Connections | the user store opens `user.db` per save (and read-only per load); the knowledge store holds one read-only connection (`user.db` as `main`, packs attached) behind a mutex | saves are rare; a pool of readers waits for a measured need (each would multiply the page caches) |
| Packs at start | structural check only (Talos's `application_id`, schema version, `meta.kind = pack`, every table, a complete manifest); a page-by-page `integrity_check` is `talos packs check` | `integrity_check` reads the whole file, ~100 MB for a vocabulary pack, against a 100 ms cold start; a damaged page found later fails that query, honestly |
| Seed pack | **beside the binary, not embedded**: `knowledge/seed.toml` is built by `cargo run -p talos-pack` into `target/packs/core.talospack` (126 976 B; 122 880 B before the `form_by_lexeme` index of Slice C) and installed by copying it into the data directory's `packs/`; tests build it in memory from the source (`Knowledge::from_source`) | it is the specification's fixture standing in for the core-vocabulary pack: always present it would answer next to, and collide with, the vocabulary pack of Slice C (same words, other concept ids). Packs travel apart from the binary (ADR 0006); embedding would also put 120 KB in every binary |
| Pack source format | TOML, read with the `toml` the engine already has; the format is in [KNOWLEDGE.md](KNOWLEDGE.md) §7, "Pack sources" | built in memory and written with `VACUUM INTO`: the same source gives the same bytes (a test builds twice and compares; debug and release builds agree). The bytes depend on SQLite's version and compile options, which `Cargo.lock` and `.cargo/config.toml` fix |
| Inflected forms | rules in `talos-core/src/knowledge/inflect.rs` (tables of endings, both ways), exceptions as `form` rows of a pack | a vocabulary pack extends the exceptions by writing rows (the wordnets' exception lists), not code; regular forms are not stored |

### The SQLite binding

Measured 2026-11-25 on an Apple-silicon Mac, release profile, each build
from the same tree with only the binding changed. Memory is max RSS / peak
footprint in MB, median of seven runs (five for the lookups), from
`/usr/bin/time -l`. The large pack is a synthetic one of the vocabulary
pack's shape (`talos-pack synthetic 120000`: 120 000 concepts, 173 833
English and 122 794 Spanish lexemes, 347 951 senses with glosses, 239 998
claims; 97 MB). "Lookups" is `examples/knowledge_footprint.rs` in `talos`:
the engine built as `talos ask` builds it, then 2 000 words each looked up
(senses through the inflection rules), its first sense's gloss, its words in
the other language and the sense choice.

| | stripped binary | `ask "hola"`, no data directory | `ask "hola"`, large pack attached | 2 000 lookups, large pack | one lookup, p50 / p99 |
|---|---|---|---|---|---|
| before (no store) | 2 793 440 B | 7.6 / 5.6 | — | — | — |
| `--no-default-features` (store compiled out) | 2 793 440 B | — | — | — | — |
| bundled, `libsqlite3-sys`'s default options | 3 733 104 B | 7.5 / 5.4 | 8.2 / 5.5 | 9.2 / 6.5 | 190 / 320 µs |
| bundled, trimmed options (**chosen**) | 3 413 760 B | 7.7 / 5.7 | 8.3 / 5.7 | 8.7 / 6.1 | 56 / 110 µs |
| the same with FTS5 kept | 3 530 624 B | — | — | — | — |
| system library (`/usr/lib/libsqlite3.dylib`) | 2 862 256 B + the library | 7.7 / 5.7 | 8.2 / 5.9 | 8.7 / 6.5 | 72 / 124 µs |

Run-to-run spread is about ±0.3 MB. `ask "hola"` with the seed pack
attached: 8.3 / 5.7. Wall time of `ask "hola"` (21 runs, median, including
process start and the `date` call): 64.9 ms before, 64.8 ms with the store
and no data directory, 68.8 ms with the large pack attached.

Reading it:

- **Memory is far inside the budget.** With a 97 MB pack attached and 2 000
  lookups done, the process stays under 9 MB resident (budget: 20 MB with
  one large pack). Packs are attached with a 256 KiB page cache each and no
  mmap, so what the pack holds does not become resident.
- **The trimmed build is 3× faster per lookup** than the default options.
  Most likely STAT4, which the default build enables: with `sqlite_stat4`
  in the pack the planner re-prepares a statement whenever a bound value
  could change its plan, so the cached statements are not reused. Not
  verified beyond this comparison.
- **The system library** is the smallest binary, but it is a run-time
  dependency whose version and options vary by host, and the static musl
  build for the Pi cannot use it.
- Trimmed: FTS3/4/5, R*Tree, DBSTAT, STAT4, SOUNDEX, column metadata and
  extension loading undefined; JSON, deprecated APIs, shared cache, the
  progress callback, UTF-16, `sqlite3_complete`, `get_table` and TCL
  variables omitted; memory statistics off; double-quoted strings refused;
  no expression-depth limit. All are options SQLite supports with the
  amalgamation.

Reproduce with `cargo build --release -p talos --example knowledge_footprint`,
`talos-pack synthetic 120000 DIR/packs/synthetic.talospack`, a word list
(`sqlite3 PACK "select lemma from lexeme where lang='en' limit 2000"`) and
`TALOS_DATA_DIR=DIR /usr/bin/time -l target/release/examples/knowledge_footprint WORDS`.

Not measured: the Raspberry Pi (the cross build of the bundled SQLite with
`cargo zigbuild` was not run here), a cold OS page cache, and Spanish
lookups at scale.

### Where the user's data lives (from v0.3)

| | |
|---|---|
| `user.db` | preferences (`preference`: key, value) and the user-correction table (`correction`, `correction_slot`), next to the knowledge tables; not created until the first save; read read-only |
| `user.toml` | read once when there is no `user.db`, written into a new `user.db`, then renamed `user.toml.migrated` (never deleted; if that name is taken it stays as it is). A `user.toml` that cannot be read creates no `user.db` and is reported like any unreadable store. Once `user.db` exists a `user.toml` is ignored |
| `packs/*.talospack` | attached at start in file-name order, read-only and immutable; up to ten (SQLite's limit with `user.db` as `main`); a pack refused is reported on stderr (`talos ask`, `chat`, `serve`) and in `/health`'s `failures` |

The guarantees are those of v0.2: an ordinary turn writes nothing (tests:
`reading_writes_nothing`, `a_fresh_data_directory_gets_nothing_written_by_a_question`,
and the engine's own count of saves), an unreadable store is never
overwritten, and tests never touch the user's home (in-memory stores, the
seed built in memory, temporary directories).

## Decisions taken in v0.3 Slice C (the vocabulary pack)

The vocabulary pack is built from the WordNet family
([wordnet-sources.md](research/wordnet-sources.md) §2.3.1, §4) by
`talos-pack wordnet` (`tools/pack/src/wordnet.rs`). Its sources and the
built pack are not committed.

### The vocabulary pack

**Getting the sources** (once; about 21 MB). Put the three files in one
directory, at its top level or one directory down (`DIR/wordnet-en/…` works):

| File | From | SHA-256 |
|---|---|---|
| `english-wordnet-2025-plus.xml.gz` | Open English WordNet 2025+, https://en-word.net/static/ (downloads page: https://en-word.net/downloads) | `31f4af16c54b532fd5484d4cc33aee588a31bb5b70683ae8197842fde5b586bc` |
| `ili-map-pwn30.tab` | the CILI repository, https://github.com/globalwordnet/cili (top level) | `13b6741395a8bdf20e5a19686ffa49167653696d4f523db17a79dce3211a6627` |
| `wn-data-spa.tab` | the Open Multilingual Wordnet data, https://github.com/bond-lab/omw-data, `wns/mcr/` | `0ec37ce94ee2acc63ad6b120e1051f9841d172778cc24adc0023fb8dccf0a7cd` |

**Building it:**

```sh
cargo run --release -p talos-pack -- wordnet DIR                 # → target/packs/vocabulary.talospack
cargo run --release -p talos-pack -- wordnet DIR OUT.talospack   # elsewhere
cargo run -p talos -- packs check target/packs/vocabulary.talospack
cargo test -p talos-pack --test vocabulary                       # properties of the built pack
cargo run --release -p talos-pack --example vocabulary_sample    # everyday words, for reading
```

The builder checks each file's SHA-256 and refuses any other file unless
given `--unverified` (the manifest then says `source_files_verified = no`).
Beside the pack it writes `vocabulary.NOTICE` (the notice, also in the
manifest) and `vocabulary.report.txt` (every count, and every row dropped
with the reason). The same files give the same bytes (checked: two builds,
`cmp`); the XML is streamed. To install the pack, copy the `.talospack` and
its `NOTICE` into the data directory's `packs/`. The tests of
`tools/pack/tests/vocabulary.rs` read the built pack (or
`$TALOS_VOCABULARY_PACK`) and pass, saying `SKIPPED`, when there is none.

| Question | Taken | Note |
|---|---|---|
| Reading the XML | `quick-xml` 0.42 (MIT; one dependency, `memchr`), streaming; `flate2` with its pure-Rust backend for the gzip; `sha2` for the checksums | dev-time only, in `talos-pack`; nothing reaches the `talos` binary. All permissive (MIT, Apache-2.0, Zlib, 0BSD, Unlicense), within `deny.toml`. 16 crates in `Cargo.lock`, 9 of them `sha2`'s |
| Concept ids | the CILI id (`i46360`) when the synset has one, else the synset id (`oewn-00002935-r`, 3 213 synsets with `ili="in"`) | as written in wordnet-sources.md §4; no prefix: both forms are self-identifying |
| English sense order | the order of the `Sense` elements in a `LexicalEntry`, which is WordNet's sense order, commonest first (no tag counts are in the LMF file). Entries with the same lemma and part of speech (`a` and `s` are both `adj`) are one lexeme, senses in document order (61 merged) | |
| One typed word, several lexemes | lexemes sharing a match form ("dog" n/v, "CAT"/"cat", "árbol"/"arbol"): the main one keeps its ranks, the others' start at 1 (15 352 lexemes). Main: most senses, a noun's counted twice, then lower case | without it, "cat" tied CAT scan with the animal and every noun–verb pair tied at rank 0, so sense choice always asked; now "tree" is the plant, "run" the verb |
| Spanish sense order | the Spanish file has none. A Spanish sense borrows the rank of its concept in the word's *pivot*, the English word found on most of its concepts (same case, then best ranks, then most senses); a concept the pivot does not name ranks after all the pivot's senses | a guess from the only signal there is; "coche" → car, then railcar; "pan" → bread, not Pan. Wrong for some words (below) |
| A concept's Spanish words | by that rank, then a plural of another of its words last (507 senses: "ríos" beside "río"), a feminine after its masculine, then the word most frequent in the Spanish file's own definitions and examples, then most concepts | this decides the Spanish name of a concept ("dog" → perro, not canino) |
| Glosses | the synset's definition on every English sense (source `oewn`); the MCR definition on every Spanish sense of the concept (source `mcr-spa`); several definitions joined with "; " (4 synsets) | a gloss's source is its sense's source, as in the seed |
| Forms | the `Form` elements of the entries (the WordNet exception lists: "geese", "went", "better"), features from the part of speech and the ending (noun `pl`; verb `ger` for -ing, `pres,3sg` for -s, `pres` for am/are, else `past`; adjective `sup` for -st, else `cmp`) | the lists do not say which form a form is; "past" stands for past and participle. Some are forms the rules also give ("abetted"); they are kept as the source lists them |
| Relations | `is_a` (hypernym), `instance_of` (instance hypernym), `part_of`, `member_of`, `substance_of` (the meronyms, subject the part), `causes`, `similar_to` and `antonym_of` (symmetric, both directions as the source gives them); each pair once | `instance_of` is not `is_a` (the seed writes Pluto `is_a` planet; v0.6 decides whether its closure follows both). Antonyms are sense relations in the source and become concept claims here (7 990 links → 7 612 claims): which word of the concept was meant is lost |
| Concept kind | `entity` for a synset with an instance hypernym (7 736), else `class` | |
| `form_of` was a full scan | **an index, `form_by_lexeme ON form(lexeme_id)`, added to schema version 1** (`schema.rs`; v1 is unreleased). Nothing in `query.rs` or `senses.rs` changed | a word's stored form with given features is found through its lexeme; without the index each call scanned 260 000 forms: a whole lookup took p50 10.4 ms, p99 13 ms on this pack, now 0.36 / 1.3 ms. The index adds 3 MB to the pack |
| The word-pair lexicon | not in this slice: its source and provenance are undecided | the hook is the schema's `word_pair` table, in a pack of its own; the senses it would induce for Spanish words (KNOWLEDGE.md §7, role 2) go into this pack under a third source |

**Counts** (OEWN 2025+, MCR via OMW, 2026-11-25):

| | |
|---|---|
| concepts | 120 564 (7 736 entities; 78 215 with Spanish words) |
| lexemes | 161 814 en, 93 694 es |
| senses | 212 659 en, 145 223 es |
| glosses | every English sense (120 564 concepts, source `oewn`); 42 144 Spanish senses on 18 853 concepts (source `mcr-spa`) |
| forms | 259 959: one per lexeme plus 4 451 exceptions (1 381 `pl`, 1 233 `past`, 625 `ger`, 601 `cmp`, 593 `sup`, 16 `pres,3sg`, 2 `pres`) |
| claims | 155 315: 93 395 `is_a`, 8 599 `instance_of`, 9 194 `part_of`, 12 292 `member_of`, 826 `substance_of`, 221 `causes`, 23 176 `similar_to`, 7 612 `antonym_of` |
| pack | 76 013 568 B; `talos packs check`: ok (2.0 s) |
| builder | 9.4 s, max RSS 555 MB (release, Apple-silicon Mac) |

**Dropped**, with the reason (the report lists examples): 202 Spanish synsets
whose CILI id no OEWN synset carries (82 n, 52 a, 51 v, 17 r; none lacks a
CILI id once `-s` is tried for adjectives, which 6 429 need); 531 Spanish
definitions of synsets with no Spanish lemma and 60 of synsets lost in the
join (18 853 of 19 444 kept); 2 empty Spanish lemmas (5 had their spaces
collapsed); 16 English exception forms equal to their lemma and 7 repeated
on a merged lexeme; and what schema version 1 has no place for: 49 724
English and 1 196 Spanish examples, 44 638 pronunciations, 3 213 ILI
definitions, 39 syntactic frames, the relations `also`, `attribute`,
`domain_region`, `domain_topic`, `entails`, `exemplifies` and their
inverses (synset level, 24 942 links) and `derivation`, `pertainym`,
`exemplifies` and its inverse, `participle`, `other`, `also`, `similar`
(sense level, 114 064 links). No English entry, sense, synset or relation
was dropped for being malformed.

**Footprint with the pack attached.** Measured 2026-11-25 on the
Apple-silicon Mac, release build, as in "The SQLite binding":
`examples/knowledge_footprint.rs` (the lookup `define` and `translate` will
make: senses through the inflection rules, the first sense's gloss, its
words in the other language and the plural of the first, the sense choice;
for an unknown word, the near misses) over word lists drawn from the pack:
"common" is the 1 000 lower-case lexemes with the most senses (the heaviest
lookups), "rare" 1 000 with one sense, "typos" 500 common words with two
letters swapped. Five runs each (three for the typos); p50/p99 of one whole
lookup in one process; memory as max RSS / peak footprint.

| | p50 | p99 | max | after the lookups |
|---|---|---|---|---|
| English, common | 360 µs | 1.29–1.34 ms | 2.8 ms | 8.7–9.5 / 6.2–7.0 MB |
| English, rare | 70 µs | 0.21–0.24 ms | 0.7 ms | 8.6–8.9 / 6.2–6.5 MB |
| Spanish, common | 365 µs | 1.14–1.19 ms | 2.0 ms | 8.7–9.1 / 6.2–6.6 MB |
| Spanish, rare | 81 µs | 0.35–0.37 ms | 0.7 ms | 8.9–9.0 / 6.4–6.5 MB |
| English, typos (near misses) | 690 µs | 1.33 ms | 1.7 ms | 9.0–9.3 / 6.5–6.9 MB |
| Spanish, typos (near misses) | 880 µs | 1.53 ms | 1.7 ms | 8.8–8.9 / 6.3–6.4 MB |

`talos ask "hola"`, seven runs: 8.3–8.5 / 5.7–6.0 MB with the pack attached,
7.5–8.2 / 5.5–6.2 MB with an empty data directory; wall time, 21 runs,
median of two interleaved rounds: 66–68 ms either way (within noise; most
of it is process start and the `date` call). Budgets (ROADMAP v0.3):
resident memory ≤ 20 MB, a knowledge query p99 ≤ 10 ms; both hold, a whole
lookup of many queries included.

**Read, not measured: a sample of everyday words.**
`examples/vocabulary_sample.rs` looks up 45 English and 43 Spanish words
and prints each one's senses, glosses, translations and what the sense
choice does with no context. Read on 2026-11-25:

- Right: the first sense of every English word is its everyday one (the
  sense choice decides alone for all but "better"), the irregular forms
  resolve ("geese", "went", "children", "mice"), and the first Spanish word
  of the first sense is the expected translation for all but two ("house"
  casa, "dog" perro, "car" coche, "computer" ordenador, "money" dinero,
  "tree" árbol). Every Spanish word but one has its everyday sense first;
  the sense choice asks for casa (with *caso*, whose feminine the rules
  read it as), agua (with *aguar*), cabeza (with *cabezo*), mano (with
  *manar*) and trabajo (with *trabajar*), the right behaviour for a form
  two lemmas share.
- Wrong or doubtful: "lentamente" → *dumbly* first (the pivot guess fails
  when every English word is on one concept only); "go" and "eat" are named
  in Spanish *moverse* and *consumir* before *ir* and *comer* (equal rank,
  and the definitions use those words more); "time" → *an instance or single
  occasion* first and "bank" → *sloping land* first: WordNet's own order;
  "banco" (financial) is named in English *depository financial
  institution*, because a concept's words are ordered by rank and the
  concept is "bank"'s second sense (KNOWLEDGE.md §2: rank, then order
  written); "better" asks between *better* and the comparative of *good*.
- Errors in the Spanish source, kept as written: plural lemmas ("perros",
  "ríos", 507 senses, ordered last), unaccented duplicates ("arbol", "dia"),
  a typo ("ninzzo"), Catalan spellings ("avançat", "tràmite", "retraïdo"):
  36 lemmas have letters outside Spanish spelling, most of them proper names
  ("Curaçao", "François Rabelais"). No mojibake (a test checks).
- 76 % of the concepts with Spanish words have no Spanish gloss (18 853 of
  78 215), as the research measured: `define` in Spanish falls back for most
  words.

**Gaps.** No Spanish irregular forms: the Spanish source has no exception
lists and none were written by hand; the rules cover the regular ones, so
"fue", "tengo", "hizo" are not found (KNOWLEDGE.md, "Inflected forms").
Examples are not stored (no table). The notice's copyright lines are those
of wordnet-sources.md §4, not yet checked against each project's own
LICENSE file (§6, items 5 and 8). The Raspberry Pi and a cold page cache
were not measured.

## Decisions taken in v0.3 Slice B (define, translate, teach)

| Question | Taken | Note |
|---|---|---|
| The fallback marker | `Outcome::fallback: Option<Fallback>`, an open set of names like `Reason`; `fallback` in the `talos` object only when set; `· fallback <name>` at the end of outcome lines (golden files, `talos explain`). A handler sets it with `Context::mark_fallback`; it is kept on `Answer` and `Unable` only | values today: `word-pairs`, `word-by-word`, `relations`, `other-language` (API.md §2.2). Weather from a cache or an encyclopedia's opening will add their own |
| Without the store | `define`, `translate` and `teach` are always installed; without the `knowledge` feature (`talos-skills/knowledge`, on by default, turned on by the binary's `knowledge`) or without a store, `define` and `translate` answer "no dictionary" (`unhandled`) | the manifests, and so the classifier's corpus hash, are the same in every build; `teach` needs no store (the correction table is the engine's user data) |
| Conversation memory | `ConversationState` keeps, per conversation, the sense chosen for each word (by key; at most 64), the concepts mentioned in the last three turns and the topic word; handlers write it through `Context` (`remember_sense`, `mention`, `set_topic`), applied only on `Answer`/`Unable` | a cancelled "which one?" leaves nothing (spec 2.2); nothing is written to disk |
| Sense order | words of the turn first (they replace a remembered sense), then the remembered one, recent concepts and rank (two calls of `choose_sense`) | the specification's example «¿y el banco que da préstamos?» after «el de sentarse» needs the turn's context to win over memory; `choose_sense` itself is unchanged |
| Reading a meaning | `Context::understand`: the correction table, then the rules, a rule counting only if its skill would take it (the handler is asked in a throwaway context: fresh conversation, no engine, draws discarded, no change applied), then the classifier when it accepts outright | "six-string" matches a `calc` pattern but `calc` declines it, so it means nothing yet |
| Free-text answers | `Expects::Text` (`Expectation::text`): the answer taken as written even when it reads as a request; `Expectation::keeping` carries slots worked out before asking | "What do you mean by it?" → "toss a coin" must not toss a coin; «¿te refieres a «guitarra»?» → "sí" must still know the word |
| Wording | definitions read «Guitarra: instrumento de cuerda…» / "Guitar: a string instrument…" | the store has no grammatical gender, so Spanish articles cannot be chosen; the dictionary form needs none |
| `teach.forget` and `teach.list` | phrases only, plus honest "not yet" for facts («olvida que la ballena…», «eso no es verdad, olvídalo») and "nothing about X" for a topic | taught facts are v0.6; `teach.assert` stays out of scope (out-of-scope corpus) |

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
  conversation; a blank line starts a new one (fresh state, turn 1, and
  what is stored about the user set back to the `Given: stored`/`phrase`
  lines). All conversations use the id `golden`, so variants are
  reproducible.
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
  | `stored` | `stored name = Raúl, place = Sevilla, units = metric` (any of the three), or `stored = none`: the preferences at the start of each conversation | `none` |
  | `phrase` | `phrase = flip it -> random.coin` or `phrase = tira -> random.dice count=2 sides=20`, one per line, adding to the table; `phrase = none` empties it: the user-correction table at the start of each conversation | empty |
  | `pack` | `pack = knowledge/seed.toml` (a pack source, relative to the directory that holds `golden/`), or `pack = none`: built in memory and given to the engine as its knowledge store. A build without the `knowledge` feature skips the turns that need one (`Summary::skipped`) | `none` |

  Anything else after `Given:` is an error, so a spec line such as "Given:
  a conversation that has been in Spanish" must be expressed with turns.
  A turn with a `draw` fails unless it draws exactly what was scripted.
- **Failures** print the file, line, input, and expected against actual.
  **Regenerate deliberately** with `TALOS_BLESS=1 cargo test -p
  talos-skills --test golden`: every T block is rewritten from the actual
  output (comments, `Given:` lines and notes are kept); review the diff
  before committing.
