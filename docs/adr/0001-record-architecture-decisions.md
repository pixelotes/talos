# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-11-24

## Context

Talos is being designed before it is built, by a small team working with AI
assistants. Decisions made in conversation are lost unless written down, and
an assistant starting a new session has no memory of why something is the way
it is.

## Decision

Keep architecture decision records in `docs/adr/`, one Markdown file per
decision, numbered, in English, following the template in
[README.md](README.md). All plans and documentation live under `docs/` and are
written in English.

A record is written when a choice is expensive to reverse, constrains other
work, or rejects an obvious alternative.

## Consequences

- The reasons behind the design are reviewable and survive between sessions.
- A change of mind costs a new record, not a silent edit.
- Records must stay short, or nobody will read them.

## Alternatives considered

- **A single design document.** Good for the current picture, poor at keeping
  the reasons and the rejected options. We keep both: `DESIGN.md` describes
  what is, ADRs say why.
