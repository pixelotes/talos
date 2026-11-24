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
| Turn number | carried on `Turn` (`number`, 1-based) until the per-conversation state holder (B3) owns it | `ask` is turn 1 of conversation `ask`; `chat` counts from 1 in conversation `chat` |
| Reply variants | turn 1 uses the first variant; later turns step through them from an FNV-1a offset of the conversation id | one-shot answers are stable; FNV-1a because `std`'s hasher is not stable across releases |
| Fallback | fixed replies in `talos-core/src/fallback.rs` | replaced by the reflector in Slice D |
