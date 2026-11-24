# Architecture decision records

One file per decision that would be expensive to reverse or that someone will
later ask "why?" about. Records are short and immutable in substance: a
decision that changes gets a new record that supersedes the old one.

## Index

| # | Decision | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-clean-room-rewrite.md) | Clean-room rewrite and provenance policy | Accepted |
| [0003](0003-rust.md) | Rust as the implementation language | Accepted |
| [0004](0004-explicit-turn-pipeline.md) | An explicit turn pipeline, not layers and an event bus | Accepted |
| [0005](0005-sqlite-knowledge-store.md) | SQLite knowledge store with language-neutral concepts | Accepted |
| [0006](0006-knowledge-packs.md) | Knowledge packs as attachable read-only databases | Accepted |
| [0007](0007-declarative-skill-manifests.md) | Declarative skill manifests | Accepted |
| [0008](0008-intent-classification.md) | Rules first, an offline-trained linear classifier second | Accepted |
| [0009](0009-reflector-fallback.md) | An ELIZA-style reflector with a three-valued outcome | Accepted |
| [0010](0010-one-binary-three-front-ends.md) | One binary, three front ends, an OpenAI-compatible server | Accepted |
| [0011](0011-bilingual-by-construction.md) | Bilingual by construction | Accepted |
| [0012](0012-no-language-model.md) | No language model in the answer path | Accepted |
| [0013](0013-external-skills.md) | External skills, out of process | Accepted |
| [0014](0014-mit-licence.md) | MIT licence | Accepted |
| [0015](0015-language-detection.md) | Language detection: unambiguous words first, a two-class classifier second | Accepted |

**Proposed** means written down and awaiting the owner's review. **Accepted**
means agreed. Later states: **Superseded by NNNN**, **Rejected**.

## Template

```markdown
# NNNN. Title

- Status: Proposed | Accepted | Superseded by NNNN | Rejected
- Date: YYYY-MM-DD

## Context
What forces are at play. Facts, constraints, what hurts.

## Decision
What we will do, in the active voice.

## Consequences
What becomes easier, what becomes harder, what we now owe.

## Alternatives considered
What else was on the table and why it lost.
```
