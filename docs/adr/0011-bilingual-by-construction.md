# 0011. Bilingual by construction

- Status: Accepted
- Date: 2026-11-24

## Context

Talos must serve Spanish and English equally. Retrofitting a second language
onto a monolingual design produces partial coverage that nobody can quantify:
some replies translated, some detectors not, and measurably worse recognition
in the language that came second.

## Decision

Spanish and English are first-class from the first commit, and the design
makes it hard to add something in one language only.

- **Language is a field of the turn**, set by the caller or detected, sticky
  per conversation with hysteresis. Every stage receives it.
- **Manifests require both languages** for examples and replies; the
  validator fails otherwise, with an explicit opt-out for intents that are
  genuinely single-language.
- **Knowledge is language-neutral** at the concept level
  ([0005](0005-sqlite-knowledge-store.md)); only lexemes, forms and glosses
  are per language.
- **Slot extractors** (numbers, durations, dates, places) are implemented and
  tested for both languages together.
- **Reflector scripts** are written natively per language.
- **Evaluation batteries exist per language** and CI enforces a floor on
  each, plus a maximum gap between them.
- Talos answers in the language of the turn. It does not translate its own
  replies at run time.

Further languages are out of scope for the first major version, but nothing
in the design assumes exactly two.

## Consequences

- Coverage per language is a number, reported on every build.
- Every feature costs roughly twice the phrasing work. That is the price of
  the requirement, paid visibly.
- Mixed-language input ("pon un timer") is handled by the language-specific
  rule sets being tried with the conversation's language first and the other
  second.
- Language detection for only two languages can be simple and exact enough
  without a general-purpose detector.

## Alternatives considered

- **Build in one language, translate later.** The known failure mode.
- **Translate input to a pivot language.** Needs a translation model; loses
  nuance; violates the no-model rule.
- **Separate engines per language.** Duplicates knowledge and dialog state.
