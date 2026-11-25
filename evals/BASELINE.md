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

## With the classifier (2026-11-25, v0.2 Slice A)

Same batteries, `talos eval`, after the classifier and the command and
question work were merged. Intent model: 21 classes, 21,420 features, 520 KB;
threshold 0.6, margin 0.3. The thresholds were set by cross-validation on the
training examples and have **not** been tuned against the batteries.

| | Spanish | English | Target |
|---|---|---|---|
| Intents of the built skills (200 rows), rules only | 49.5 % | 48.0 % | |
| The same, rules and classifier | **89.5 %** (179) | **89.5 %** (179) | es ≥ 92 %, en ≥ 95 % |
| Wrong intent / no intent (of 380 rows) | 11 / 190 | 11 / 190 | |
| False accepts on the rejection set (99 rows) | 1 (1.0 %), a near miss | 1 (1.0 %), a near miss | ≤ 3 % |
| Conversation battery (66 turns): bare catch-all | 31.8 % → 25.8 % | 34.8 % → 24.2 % | ≤ 15 % by v0.5 |

The 190 rows with no intent are, for 180 of them, skills that do not exist
yet. Recognition of built skills is 5.5 points short of the English target
and 2.5 short of the Spanish one; the gap between languages is zero.

A classified turn carries no captured values, so a request accepted by the
classifier is answered with the intent's defaults ("lanza tres dados" rolls
one die). Counted as correct here, because the battery scores the intent.

## After Slice C (2026-11-25): nine skills

`timezones` and `preferences` are built, so their rows count as built skills
(250 rows per language, 25 intents). Model retrained; thresholds unchanged
and still not tuned against the batteries.

| | Spanish | English | Target |
|---|---|---|---|
| Intents of the built skills, rules only | 51.2 % | 50.4 % | |
| The same, rules and classifier | **87.2 %** (218) | **86.8 %** (217) | es ≥ 92 %, en ≥ 95 % |
| Wrong intent / no intent (of 380 rows) | 11 / 151 | 10 / 153 | |
| False accepts on the rejection set (99 rows) | 3 (3.0 %) | 2 (2.0 %) | ≤ 3 % |
| Conversation battery (66 turns): bare catch-all | 15.2 % | 12.1 % | ≤ 15 % by v0.5 |

All false accepts are near misses. The two new skills are the weakest:
`timezones.difference` 60 % in Spanish, `preferences.set` 70 % in Spanish,
`preferences.forget` and `preferences.get` 70 % in English. The side-effect
rule holds `preferences.set` and `preferences.forget` back on purpose: the
classifier may only offer them for confirmation.
