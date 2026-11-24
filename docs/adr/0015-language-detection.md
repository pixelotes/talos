# 0015. Language detection: unambiguous words first, a two-class classifier second

- Status: Accepted
- Date: 2026-11-24

## Context

Every turn needs a language, Spanish or English, before anything else runs
([0011](0011-bilingual-by-construction.md)). The design said "function words
plus character n-gram profiles" without deciding whether that is a heuristic
or a trained model. Messages are often one or two words long, which is where
heuristics are weakest, and the project already has the machinery for a small
linear classifier ([0008](0008-intent-classification.md)) and a corpus of
labelled phrases for exactly this task.

## Decision

Language is decided in this order:

1. **The caller's hint**, if present, wins outright.
2. **Unambiguous words.** A short list per language of function words and
   characters that occur in only one of the two (`the`, `what`, `el`, `qué`,
   `ñ`, `¿`). A message containing markers of one language and none of the
   other is decided here.
3. **A two-class linear classifier** over character n-grams, trained offline
   with the same tool and exported in the same format as the intent
   classifier. It returns a language only above a confidence threshold.
4. **Otherwise the conversation's current language** is kept; on a first turn,
   the configured default.

The result is sticky per conversation with hysteresis: one low-confidence or
single-word message never flips it; a confident message in the other language
does. The word-pair lexicon supplies vocabulary for recognising a word of the
other language inside a sentence, so "pon un timer" stays Spanish.

The language model file is separate from the intent model, a few kilobytes,
and gated by its own evaluation battery, including one- and two-word messages
and mixed-language ones.

## Consequences

- No new machinery: one trainer, one file format, one inference routine for
  both classifiers.
- Short messages are handled by the classifier and by stickiness instead of by
  ever-longer word lists.
- A second training corpus and battery to maintain, both small.
- Detection is still only as good as two languages allow; adding a third
  language means revisiting step 2 and retraining step 3, not redesigning.

## Alternatives considered

- **Heuristic only** (word lists and the lexicon). Explainable and
  training-free, but fragile on short messages and on words shared by both
  languages.
- **Classifier only.** Wastes certainty that a single unambiguous word gives
  for free, and makes obvious cases depend on a threshold.
- **A general-purpose language-identification library.** Solves a
  hundred-language problem at a size and accuracy profile wrong for a
  two-language one.
