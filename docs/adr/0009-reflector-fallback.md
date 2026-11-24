# 0009. An ELIZA-style reflector with a three-valued outcome

- Status: Accepted
- Date: 2026-11-24

## Context

A bounded assistant fails to recognise a large share of what people type. If
every such turn answers "I don't know how to respond to that", stand-alone use
feels broken. A pattern-reflection script in the manner of ELIZA (Weizenbaum,
1966) is a very cheap way to keep a conversation moving.

It also creates a problem. When Talos sits in front of a more capable engine,
a reflection is a deflection: the user asked for a poem and got "can you tell
me more?". The host must be able to tell a real answer from a reflection, and
a reflection from a blank.

## Decision

- Keep a **reflector** as the last stage: ranked keywords, decomposition and
  reassembly rules, pronoun reflection. One script per language, stored as
  data.
- Every outcome carries one of three statuses:
  - `Answered` — a skill produced an answer;
  - `Deflected` — the reflector matched a specific pattern;
  - `Unhandled` — Talos could not provide what was asked: nothing recognised
    the message, or a skill or the reflector recognised it and could not help
    (an unknown word, a request for advice, a command Talos cannot carry out).
    The intent or shape is kept on the outcome with a reason.

  The test between `Answered` and `Unhandled` is whether a more capable engine
  would plausibly serve the user better (amended 2026-11-24, after the
  specifications showed "only the catch-all" to be too narrow).
- Script design rules: recognise the *form* of a sentence, never imply
  understanding of its content; widen coverage of sentence shapes rather than
  add therapeutic prompts; do not reflect questions that look factual.
- Success is measured as the share of stand-alone turns ending in the bare
  catch-all.

## Consequences

- Stand-alone Talos is pleasant where it cannot be useful.
- A host can choose its policy: show deflections, or treat anything other than
  `Answered` as a reason to pass the turn on.
- The reflector can grow freely without stealing turns from a host, because
  the host sees the status.
- Scripts are original work per language; a Spanish script is written as
  Spanish, not translated from English.
- There is a standing risk of patterns that sound like comprehension. Review
  of script changes has to watch for it.

## Alternatives considered

- **No fallback, an honest "I don't know".** Correct and dull; kept as the
  `Unhandled` reply.
- **A boolean `handled`.** Cannot express the useful middle case, and forces
  either the reflector or the host to guess.
- **A small generative model for chit-chat.** Contradicts
  [0012](0012-no-language-model.md).
