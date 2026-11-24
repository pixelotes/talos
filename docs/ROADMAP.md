# Roadmap

Seven milestones from an empty repository to a 1.0 that replaces the Talos
inside Tomos. Each one ends in something that runs and can be measured.

The rule for the whole plan: **a milestone is done when its exit criteria are
measured, not when its code exists.** Numbers marked *target* are goals to be
confirmed or revised by the first real measurement.

```
M0 Foundations ─▶ v0.1 Talks ─▶ v0.2 Understands ─▶ v0.3 Knows
                                                        │
                       v1.0 Small hardware ◀─ v0.5 Tools ◀─ v0.4 Serves
```

No dates: the order and the exit criteria are the plan. Sequence can bend
where noted.

---

## M0 — Foundations

Everything needed before the first line of engine code.

**Scope**

- Review and accept (or amend) the ADRs marked *Proposed*.
- Write the **behavioural specification**: for each planned skill, example
  exchanges in both languages, including ambiguity, missing slots and refusal.
  This is what implementation works from ([ADR 0002](adr/0002-clean-room-rewrite.md)).
- Define the evaluation battery format; assemble the first batteries
  (Spanish, English, rejection set, stand-alone conversation set) from the
  owner's own corpus, after the provenance check.
- Cargo workspace skeleton, CI (build, test, lint, licence check of
  dependencies), cross-compilation to ARM64 proven with a "hello" binary.
- Spike: packs attached vs. merged ([ADR 0006](adr/0006-knowledge-packs.md)).
- Verify the core vocabulary source: licence of the Spanish wordnet and its
  gloss coverage ([ADR 0005](adr/0005-sqlite-knowledge-store.md)).

**Exit criteria**

- The core vocabulary source is confirmed usable, with its licence and Spanish
  gloss coverage written down.

- All ADRs are Accepted, Rejected or superseded; none left Proposed.
- The specification covers every skill listed for v0.1–v0.3.
- Batteries exist with at least 150 phrases per language and a rejection set
  of at least 50.
- CI is green and produces an ARM64 artefact.
- The pack spike has a written result and ADR 0006 is updated with it.

---

## v0.1 — A skeleton that talks

The pipeline end to end, with rules only.

**Scope**

- `Turn`, `Outcome`, the pipeline with trace.
- Normalisation; language detection with per-conversation stickiness.
- Manifest format, loader, validator (`talos skills check`), embedded
  built-ins.
- Rule stage; template rendering with deterministic variants.
- Reflector with first Spanish and English scripts; three-valued status.
- `talos chat`, `talos ask`, `talos explain`.
- Skills: `smalltalk`, `identity`, `help`, `clock`, `calc`, `random`,
  `fortune`.
- Golden-transcript test harness with an injected clock.

**Exit criteria**

- A stranger can hold a short conversation in either language at a terminal.
- Identical input and state produce byte-identical output across runs.
- `talos explain` shows every stage's decision.
- Resident memory at rest ≤ 8 MB; rule-matched turn p99 ≤ 2 ms on a laptop
  (*target*).
- Binary ≤ 6 MB stripped, without the server feature (*target*).

---

## v0.2 — Understands paraphrase

Recognition that generalises, and multi-turn exchanges.

**Scope**

- Offline trainer (`tools/train`) and the model file format.
- Classifier inference in Rust; thresholds and margin; out-of-scope class.
- Typed slot extractors for both languages.
- Pending dialog: expectations, expiry, cancellation, release.
- User-correction table.
- `talos eval` with accuracy, false-accept rate and catch-all rate; CI floors.
- `timezones` and `preferences` skills (they exercise slots and state).

**Exit criteria**

- Intent accuracy on held-out batteries: English ≥ 95 %, Spanish ≥ 92 %, gap
  ≤ 4 points (*target*; the Spanish figure is deliberately above the 88 % the
  previous implementation plateaued at).
- False accepts on the rejection set ≤ 3 % (*target*).
- Model file ≤ 2 MB; classifier stage p99 ≤ 1 ms on a laptop (*target*).
- A slot-filling exchange and its cancellation pass as golden transcripts in
  both languages.

---

## v0.3 — Knows things

The semantic store. This is the milestone that makes it Talos.

**Scope**

- Schema, migrations, connection handling, `user.db`.
- Query layer: definitions, is-a, part-of, kinds-of, causes, neighbourhood;
  bounded reasoning with paths; confidence aggregation.
- Polysemy: ranking, context re-ranking, asking which sense.
- Teaching: teach, correct, forget, list, export/import; contradiction
  detection; audit.
- Skills: `define`, `relations`, `teach`, `translate`.
- Pack format and `talos packs`; a hand-written seed pack; then the core
  vocabulary pack built from the WordNet family (English and Spanish, aligned).
- The word-pair lexicon as a pack: cleaning pass, translation fallback, and a
  measured trial of inducing Spanish senses from it (precision on a reviewed
  sample decides whether induced senses ship).
- Spanish glosses: hand-written for the core subset; build-time translation of
  English glosses for the rest, sampled and reviewed, stored as a separate
  lower-confidence source.
- A semantic evaluation battery (questions with expected answers and paths).

**Exit criteria**

- Something taught in one language is answerable in the other.
- Derived answers state their path; negative answers distinguish "contradicted"
  from "unknown".
- Semantic battery ≥ 90 % (*target*).
- With a pack of at least 100,000 concepts attached: resident memory ≤ 20 MB,
  knowledge query p99 ≤ 10 ms on a laptop (*target*).
- No disk write on a turn that does not teach.

---

## v0.4 — Serves

Talos as a backend, and the first real integration.

**Scope**

- `talos serve`: `/health`, `/v1/models`, `/v1/chat/completions` (with
  streaming), with the `talos` extension object.
- Conversation identity: explicit key, `user` field, transcript fingerprint;
  bounded dialog-state store.
- Token authentication for non-loopback binds.
- Compatibility tests against at least two unmodified OpenAI-style clients.
- Tomos talks to Talos as it would to any OpenAI-compatible endpoint and reads
  the `talos` object to decide whether to fall through. The change lives in
  Tomos and is a client, not an adapter: no shared bus, no shared skills, no
  callback from Talos into Tomos.

**Exit criteria**

- An off-the-shelf OpenAI client holds a multi-turn exchange, including an
  open question, with no Talos-specific configuration.
- Tomos runs its full-integration and Talos-only modes against the new server
  and falls through correctly on `Deflected` and `Unhandled`.
- Server overhead ≤ 1 ms per request; memory with the server feature ≤ 12 MB
  at rest without packs (*target*).

*Can move earlier:* the server depends only on v0.1. If integration feedback
is wanted sooner, do v0.4 right after v0.1 and keep the integration experimental.

---

## v0.5 — Tools

Parity with what the old Talos could do for a user, plus the door for more.

**Scope**

- Capabilities and their configuration; the `net` feature.
- `weather` (with place resolution and offline degradation).
- `search` (sourced extract; configurable backend, including a host-provided
  one).
- `games`: chess with conversation-scoped state; hangman or guess-the-number.
- Decide the external skill transport — own JSON protocol, MCP, or both
  ([ADR 0013](adr/0013-external-skills.md)) — then implement it with
  allow-list and timeouts, and a reference external skill.
- Reflector coverage pass, driven by the stand-alone battery.
- Parity checklist against the old Talos's user-visible capabilities, each
  item marked done, dropped on purpose, or deferred.

**Exit criteria**

- Every item on the parity checklist has a decision.
- With the network unplugged, every skill either works or says clearly why it
  cannot; no turn hangs beyond its timeout.
- Stand-alone battery: bare catch-all on ≤ 15 % of turns (*target*).
- An external skill written in another language answers through Talos in both
  languages.

---

## v1.0 — Small hardware

Prove the claim the project exists for, and freeze the interfaces.

**Scope**

- Measurement on real devices: a Raspberry Pi 4 or 5 (ARM64) and one
  smaller board (Zero 2 W class). Memory at rest, latency per stage, cold
  start, behaviour under an SD card.
- Fix whatever the measurements expose.
- Release builds for Linux ARM64/ARMv7/x86-64 and macOS; a container image;
  an install script or packages.
- Freeze and version: the `talos` extension object, manifest
  format, external skill protocol, pack schema.
- User documentation: install, configure, teach, write a skill, build a pack.
- Tomos switches its default to the new Talos; the old one is removed from
  Tomos.

**Exit criteria**

- The budgets in [DESIGN.md §8](DESIGN.md) are met **on the Raspberry Pi**,
  with the measurements published in the repository.
- A clean device goes from download to first answer in under five minutes
  following the documentation.
- Tomos no longer contains the old Talos.
- All four interfaces above carry a version number and a compatibility
  statement.

---

## After 1.0

Not planned, only noted so they have somewhere to live:

- **An entities pack from Wikidata** — places, people, works. The model
  already accommodates it; it is a pack to build, not a redesign.
- **Speaking first** — timers, reminders, "tell me when…". Needs an
  interruption design and a channel to the host.
- **Voice** — as a front end; speech recognition stays outside the engine.
- **Embeddings as a ranking signal** for sense selection and search.
- **More languages.**
- **A sandbox for external skills** (WebAssembly) if untrusted skills become
  a use case.
- **On-device retraining** of the classifier.
- **Free-text fact extraction with review**, so Talos can propose claims from
  a document and the user approves them.

## Risks

| Risk | Where it bites | Mitigation |
|---|---|---|
| Spanish recognition plateaus below target | v0.2 | corpus work before model work; per-language floors in CI from day one |
| Cross-pack queries too slow | v0.3 | spike in M0; merge-on-install fallback already designed |
| Spanish definitions are scarce in the open wordnet (licence is fine; coverage is not) | v0.3 | hand-written glosses for a core subset plus build-time translation of the rest (decided); translation quality checked by sampling |
| Transcript fingerprinting fails with real clients | v0.4 | explicit conversation key remains the reliable path |
| Rust slows iteration more than expected | all | keep the engine small; revisit ADR 0003 after v0.1 with real data |
| Scope creep toward "companion" features | all | non-goals in DESIGN.md; ADR 0012 |
