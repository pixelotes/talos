# 0007. Declarative skill manifests

- Status: Accepted
- Date: 2026-11-24

## Context

What an assistant understands tends to be scattered: detection logic in a
router, training phrases in a corpus file, reply strings inside handlers, a
help text maintained by hand. Each copy drifts from the others. Routing order
ends up encoded in the position of `if` statements, and a second language
means touching every one of those places.

## Decision

Each skill is described by one TOML manifest that is the single source for:

- its intents, with an explicit numeric priority;
- training examples per language;
- optional certain-match patterns per language;
- typed slots;
- reply templates per language, with named variables;
- a one-line summary used to generate the help answer;
- the capabilities it requires.

Handlers contain logic only. They receive typed slots and return a template
identifier with variables; they never build sentences and never inspect raw
text for routing.

A validator (`talos skills check`) enforces the format, the presence of both
languages, and consistency between manifest and handler. Built-in manifests
are embedded in the binary at build time.

## Consequences

- Rules, classifier data, replies and help cannot drift apart.
- Adding a language is adding keys to manifests, not editing code.
- Routing order is reviewable as data.
- Non-programmers can improve coverage by editing examples and replies.
- The manifest format is an interface and must be versioned.
- Skills with unusual needs may feel constrained by templates; the escape
  hatch is a reply variable containing pre-rendered text, used sparingly.

## Alternatives considered

- **Detection in code, per skill.** Flexible, and exactly what produces
  drift and implicit ordering.
- **A central intents file plus separate reply catalogue.** Better than code,
  but splits one skill across files that must be kept in step.
- **Fluent or gettext for replies.** More powerful plural and gender handling
  than needed for two languages at this scale; can be adopted later behind the
  same template identifiers.
