# Baseline: rules only

Recorded 2026-11-25 at the close of v0.1 (commit `94c2e1c`), with
`cargo run --release -p talos -- eval`. Recognition is by rules alone; there
is no classifier yet. **A baseline for v0.2, not a gate.**

## Intent batteries

| | Spanish | English |
|---|---|---|
| All intents (380 rows per language) | 99 correct, 26.1 % | 96 correct, 25.3 % |
| Intents of the seven built skills (200 rows, 20 intents) | 99 correct, 49.5 % | 96 correct, 48.0 % |

The 180 remaining rows per language belong to skills that do not exist yet
(`define`, `relations`, `teach`, `translate`, `preferences`, `timezones`) and
score zero by construction. The second row is the meaningful one: with rules
alone, about half of the held-out phrasings of a built skill are recognised.

Per intent, built skills only (correct / rows):

| Intent | es | en |
|---|---|---|
| `calc.arithmetic` | 7/10 | 7/10 |
| `calc.convert` | 9/10 | 7/10 |
| `calc.percent` | 5/10 | 7/10 |
| `clock.date` | 3/10 | 3/9 |
| `clock.now` | 5/10 | 7/10 |
| `clock.until` | 6/10 | 5/10 |
| `clock.weekday` | 5/10 | 6/11 |
| `fortune.tell` | 3/10 | 4/10 |
| `help.capabilities` | 5/10 | 3/10 |
| `help.skill` | 3/10 | 2/10 |
| `identity.nature` | 6/10 | 2/10 |
| `identity.who` | 6/10 | 7/10 |
| `random.coin` | 3/10 | 4/10 |
| `random.dice` | 5/9 | 4/10 |
| `random.number` | 4/10 | 4/10 |
| `random.pick` | 3/9 | 4/10 |
| `smalltalk.bye` | 6/11 | 5/10 |
| `smalltalk.greet` | 4/10 | 6/9 |
| `smalltalk.how_are_you` | 6/11 | 4/11 |
| `smalltalk.thanks` | 5/10 | 5/10 |

## Rejection sets

99 rows per language: text no skill should accept.

| | Spanish | English |
|---|---|---|
| False accepts | 0 (0.0 %) | 0 (0.0 %) |
| Status as the battery expects | 78 (78.8 %) | 80 (80.8 %) |

No false accept in any category (conversational, future skill, generative,
near miss, noise, open question, statement). The status mismatches are
between `deflected` and `unhandled`: the reflector's boundary, not a skill
answering. They were not broken down further here.

## Reading

- Rules are precise and narrow: nothing is accepted wrongly, half of the
  paraphrases are missed. That is the gap the classifier of v0.2 exists to
  close (targets: English ≥ 95 %, Spanish ≥ 92 %, false accepts ≤ 3 %).
- About a tenth of the intent rows are textually identical to an example in
  the specifications (see `docs/plans/M0.md`, follow-ups); they are not
  separated here.
