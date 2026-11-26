# Roadmap

Seven milestones from an empty repository to a 1.0 that replaces the Talos
inside Tomos. Each one ends in something that runs and can be measured.

The rule for the whole plan: **a milestone is done when its exit criteria are
measured, not when its code exists.** Numbers marked *target* are goals to be
confirmed or revised by the first real measurement.

```
M0 Foundations ─▶ v0.1 Talks ─▶ v0.2 Understands ─▶ v0.3 Dictionary
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
- Command shortcuts (`/calc 1 + 2`, `/tiempo Valencia`) declared in the
  manifests ([ADR 0016](adr/0016-command-shortcuts.md)); `/help` lists them.
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

## v0.3 — A dictionary

The store and its first two uses: defining a word and translating one. Split
from the original v0.3 by the owner on 2026-11-25: the assistant is used for
direct requests, and reasoning over relations and teaching facts are not
among them yet. They are noted under "After 1.0", on the same store.

**Scope**

- Schema, migrations, connection handling, `user.db` — the schema as designed
  in [KNOWLEDGE.md](KNOWLEDGE.md), relations included, so that reasoning later adds
  queries and not a new store. Preferences and corrections move from
  `user.toml` into it.
- Query layer for what this milestone asks: a word to its concepts, a
  concept's gloss, a concept's words in the other language.
- Polysemy: ranking, context re-ranking, asking which sense.
- Skills: `define` and `translate` (words and a table of stock phrases).
- `teach.phrase` only — "when I say X, I mean Y" — which gives the correction
  table of v0.2 a spoken way in, with listing and removing.
- Pack format and `talos packs`; a hand-written seed pack; then the core
  vocabulary pack built from the WordNet family (English and Spanish,
  aligned), relations loaded even though nothing queries them yet.
- The word-pair lexicon as a pack: cleaning pass, translation fallback.
- Spanish glosses: those the Spanish wordnet has (about a quarter of its
  concepts) and Wikidata's Spanish descriptions for 18 516 more (48 % in
  all). For the rest, the cheap fallbacks, each marked as such: the
  English gloss, and from v0.5 the opening of the Wikipedia article.
  Hand-written and build-time-translated glosses are not planned before 1.0.
- An evaluation battery for definitions and translations.

**Exit criteria**

- A word is defined in the conversation's language when a gloss exists, and
  the reply says so when it falls back.
- A word with several senses is asked about, and the answer picks one.
- A word is translated in both directions; an unknown one is said to be
  unknown.
- With a pack of at least 100,000 concepts attached: resident memory ≤ 20 MB,
  knowledge query p99 ≤ 10 ms on a laptop (*target*).
- No disk write on a turn that changes nothing.

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

Parity with what the old Talos could do for a user. Every skill is built in:
Talos stands on its own ([ADR 0013](adr/0013-external-skills.md), amended).

**Scope**

- Capabilities and their configuration; the `net` feature.
- `weather` (with place resolution and offline degradation).
- `search` (sourced extract; configurable backend).
- `news` (today's headlines from a configured source, with their origin; no
  summary). To be specified before it is built.
- `wikipedia` (the article for a subject, in the conversation's language:
  its opening and an **extractive** summary — sentences chosen from the
  article and quoted as written, with the link). It is also what `define`
  falls back to when the store has no entry and the network is allowed. To
  be specified before it is built.
- Shared by `search`, `news` and `wikipedia`: reading a web page down to its
  text, and choosing sentences from it. The owner's Dédalo project already
  does both without a model and is the reference to reimplement from: a
  page reader (fetch, strip markup, extract by selector) and three
  summarisers — Luhn, Luhn+ (stop-word filter, length normalisation, lead
  bias, redundancy penalty) and TextRank. Which summariser Talos ships is
  decided by comparing them on Spanish and English articles.
- `games`: chess with conversation-scoped state; hangman or guess-the-number.
- Reflector coverage pass, driven by the stand-alone battery.
- Parity checklist against the old Talos's user-visible capabilities, each
  item marked done, dropped on purpose, or deferred.

**Exit criteria**

- Every item on the parity checklist has a decision.
- With the network unplugged, every skill either works or says clearly why it
  cannot; no turn hangs beyond its timeout.
- Stand-alone battery: bare catch-all on ≤ 15 % of turns (*target*).

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
  format, pack schema.
- User documentation: install, configure, teach, write a skill, build a pack.
- Tomos switches its default to the new Talos; the old one is removed from
  Tomos.

**Exit criteria**

- The budgets in [DESIGN.md §8](DESIGN.md) are met **on the Raspberry Pi**,
  with the measurements published in the repository.
- A clean device goes from download to first answer in under five minutes
  following the documentation.
- Tomos no longer contains the old Talos.
- All three interfaces above carry a version number and a compatibility
  statement.

---

## After 1.0

Not planned, only noted so they have somewhere to live:

- **Knowing things** — reasoning over the relations the vocabulary pack
  already holds, and teaching facts. Moved here by the owner on 2026-11-26:
  what the owner asks of Talos is direct requests and lookups, and none of them
  needs it. The relations stay in the pack and are used as data (naming a
  sense by its class, the "it is a kind of…" fallback of `define`); what
  waits is the machinery. Its scope, kept for when it is taken up:

  - Query layer: is-a, part-of, kinds-of, causes, neighbourhood; bounded
    reasoning with paths; confidence aggregation.
  - Teaching: teach, correct, forget, list, export/import; contradiction
    detection; audit.
  - Skills: `relations`, and the rest of `teach`.
  - A measured trial of inducing Spanish senses from the word-pair lexicon
    (precision on a reviewed sample decides whether induced senses ship).
  - Spanish glosses: hand-written for the core subset; build-time translation
    of English glosses for the rest, sampled and reviewed, stored as a separate
    lower-confidence source.
  - A semantic evaluation battery (questions with expected answers and paths).
- **An entities pack from Wikidata** — places, people, works. The model
  already accommodates it; it is a pack to build, not a redesign.
- **Speaking first** — timers, alarms and reminders ("avísame en diez
  minutos", "recuérdame mañana que llame a Ana"). Moved here by the owner on
  2026-11-25. Talos would say something nobody just asked for, which needs a
  scheduler, items stored on disk and a way to reach the user: easy at a
  terminal, not available over the OpenAI-style interface, where the host
  would have to hold a stream of events open or be called back. It may end
  up working from `talos chat` only. How to do it is undecided.
- **Calendar** — two different things. *Asking* ("what do I have today?")
  is an ordinary request and needs no scheduler: only a source of events.
  *Being told* about an event builds on the delivery channel above. For the
  source, either Talos reads a calendar itself, or the owner's Dédalo
  workflow engine serves it: a Dédalo flow called over HTTP and returning
  structured data is exactly an external skill
  ([ADR 0013](adr/0013-external-skills.md)), and would open the same door
  for anything else Dédalo can already fetch. Undecided.
- **Home control** — "close the dining-room blinds", "turn on the air in the
  bedroom". Far off. It needs a model of rooms and devices, a connection to
  whatever runs the home, and confirmation rules for actions with effects.
  This is where external integration
  ([ADR 0013](adr/0013-external-skills.md)) would be reconsidered.
- **Voice** — as a front end; speech recognition stays outside the engine.
- **Embeddings as a ranking signal** for sense selection and search.
- **More languages.**
- **External skills** — handlers outside the process, over Talos's own
  protocol or MCP, each with its manifest
  ([ADR 0013](adr/0013-external-skills.md)); and a sandbox for them
  (WebAssembly) if untrusted skills become a use case.
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
