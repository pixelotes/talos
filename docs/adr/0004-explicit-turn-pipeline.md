# 0004. An explicit turn pipeline, not layers and an event bus

- Status: Accepted
- Date: 2026-11-24

## Context

A common shape for assistants of this kind is a stack of conceptual layers
that communicate over a publish/subscribe bus. It is attractive because
modules are decoupled and new ones "just subscribe".

Experience with that shape showed its costs:

- Control flow is implicit. To know what happens to a message you must find
  every subscriber of every event it triggers.
- Context is easy to lose. When any publisher may forget to attach the turn's
  language or conversation, some eventually do.
- Ordering lives in incidental places such as registration order or line
  numbers.
- A single turn fans out into dozens of events, each a place to log, fail or
  allocate.
- "Did anything handle this?" has no direct answer; it has to be inferred
  from which events fired.

## Decision

A turn is processed by a fixed, ordered sequence of stages, each a function
that takes the turn and returns a typed result:

normalise → language → pending dialog → rules → classifier → dispatch →
render, with the reflector as the fallback.

A safety check for messages of distress sits between language detection and
the pending dialog (added 2026-11-24, after the specifications showed that a
check placed in the fallback could be pre-empted by an open question).

- The `Turn` is an owned value passed explicitly. There is no ambient context.
- There is no event bus in the engine.
- Order and priority are explicit: stage order in one function, rule priority
  as a number in a manifest.
- Each stage appends to a trace that can be returned to the caller.
- The engine returns exactly one `Outcome` per turn, with an explicit status.

## Consequences

- The whole path of a message can be read top to bottom in one place and
  shown to a user with `talos explain`.
- Stages are testable in isolation; golden transcripts are cheap.
- Cross-cutting observers (logging, metrics) hook the trace, not the flow.
- Adding a new *kind* of processing means editing the pipeline deliberately.
  That friction is intended.
- Features that need to react to things outside a turn (timers, notifications)
  have no natural home here and need their own design.

## Alternatives considered

- **Layers with an event bus.** Rejected for the reasons above.
- **A plugin graph with declared dependencies.** More flexible than needed;
  reintroduces implicit ordering.
- **Middleware chain (each stage calls `next`).** Equivalent in power, harder
  to trace and to short-circuit cleanly than a flat sequence returning values.
