# Talos

A deterministic, semantic assistant that answers without a language model.

Talos understands a bounded set of things well, answers them the same way every
time, and says so when a message is outside what it knows. It is built to run
on small hardware: one static binary, one SQLite file, a few megabytes of
memory.

> **Status: design phase.** There is no code yet. This repository currently
> holds the design, the decision records and the roadmap.

## What it is

- **Deterministic.** Same input and same state give the same answer. No
  sampling, no model in the answer path.
- **Semantic.** A knowledge store of concepts, senses and relations that Talos
  can query, reason over, explain, and be taught.
- **Light.** Target: under 20 MB resident at rest, single-digit milliseconds
  per offline turn, cold start under 100 ms.
- **Bilingual.** Spanish and English, by construction rather than by
  translation of templates.
- **Standalone.** Runs as an interactive chat, a one-shot command, or a server
  with an OpenAI-compatible API.
- **Honest.** Every answer says whether Talos actually handled the message, so
  a host application can pass the turn on to something else.

## What it is not

- Not a language model, and not a wrapper around one.
- Not a companion: no mood tracking, no personality engine, no unsolicited
  messages.
- Not multi-tenant: one owner, many conversations.

## Documentation

| Document | What it covers |
|---|---|
| [docs/DESIGN.md](docs/DESIGN.md) | Architecture: the turn pipeline, components, budgets |
| [docs/KNOWLEDGE.md](docs/KNOWLEDGE.md) | The knowledge store: schema, reasoning, packs |
| [docs/SKILLS.md](docs/SKILLS.md) | Skills, manifests, the reflector fallback |
| [docs/API.md](docs/API.md) | CLI and HTTP interfaces |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones and exit criteria |
| [docs/adr/](docs/adr/README.md) | Architecture decision records |

## Origin

Talos is a from-scratch redesign of the idea behind an earlier component of the
same name inside the Tomos project. It shares the concept and none of the code.
See [ADR 0002](docs/adr/0002-clean-room-rewrite.md).

## Licence

[MIT](LICENSE). Knowledge packs built from third-party sources carry their own
licences. See [ADR 0014](docs/adr/0014-mit-licence.md).
