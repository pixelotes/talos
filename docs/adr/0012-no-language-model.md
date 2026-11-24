# 0012. No language model in the answer path

- Status: Accepted
- Date: 2026-11-24

## Context

It is tempting to improve a deterministic assistant by passing its output
through a small language model for phrasing, or by using one to interpret
input. Either move changes what the project is: answers stop being
reproducible, a model must be loaded or reachable, latency grows by orders of
magnitude, and a new failure mode appears. If a model is available and
affordable, it writes better than any template and should simply answer the
question itself.

The value of Talos is the opposite corner: what can be answered identically
every time, in milliseconds, on hardware where a model does not fit.

## Decision

No language model, large or small, local or remote, participates in
understanding a message or producing a reply inside Talos.

- Understanding is rules plus a linear classifier
  ([0008](0008-intent-classification.md)).
- Replies are templates filled with facts.
- When a request needs generated text (a poem, a summary, a plan, free
  translation), Talos returns `Unhandled` and says plainly that this is not
  something it does.

Talos may be *placed in front of* a model by a host application. That
composition is the host's business and uses the outcome status; Talos itself
does not call the model.

### The one door left open

The rule binds the core and everything shipped in the binary. It does not
forbid a user from installing an **external skill** that happens to use a
model — for example an ephemeral specialist that starts, performs one
extraction, and exits.

- No built-in skill uses a model. A default installation never downloads or
  starts one.
- Rules, classifier, knowledge store, templates and reflector never use one,
  whatever is installed.
- An external skill that produces model-generated content must declare
  `generated = true` in its manifest. Its outcomes carry `generated: true`,
  so a host or a user can tell a reproducible answer from one that is not.
- The determinism guarantee is therefore stated precisely: every outcome
  without the `generated` mark is reproducible.

Talos does not manage model processes, files or lifecycles. That belongs
entirely to the external skill.

## Consequences

- Determinism, the resource budget and offline operation are protected by a
  rule that is easy to check.
- The boundary of what Talos can do is sharp, and the outcome status makes it
  usable.
- Some features are permanently out of scope for the core. Ephemeral
  specialist models remain possible as external skills
  ([0013](0013-external-skills.md)) chosen by the user. The possibility is
  kept deliberately even if it is never used; it costs one manifest field and
  one outcome field.
- Reply quality depends on template craft.

## Alternatives considered

- **Model-polished replies with fact validation.** Pays the full cost of a
  model to improve wording, and keeps the template as a fallback anyway.
- **A small model for intent detection.** See the alternatives in 0008.
- **Strictly no model anywhere, external skills included.** Simplest to
  state, but closes off uses such as proposing facts from a document for the
  user to approve, for no gain to the core.
- **A specialist-model runner inside the core.** Puts process management,
  timeouts and model files into the part of Talos that must stay small, for a
  narrow use case that an external skill serves equally well.
