# The classifier models

Talos has two small linear classifiers, built from the same code and stored
in the same file format ([ADR 0008](adr/0008-intent-classification.md),
[ADR 0015](adr/0015-language-detection.md)):

- the **intent model** (`models/intent.model`), stage 4 of the pipeline: one
  class per intent that has examples, plus `out-of-scope`;
- the **language model** (`models/language.model`), step 3 of language
  detection: `es` and `en`.

This document is the reference for the trainer, the file format and how the
engine uses them. The code is the authority: `crates/talos-core/src/classifier/`
(features, file, inference) and `tools/train/` (training).

## 1. Training data

- **Intent examples** are the `[intent.examples]` of every built-in manifest,
  in both languages (both-or-neither, as for everything in a manifest). They
  are written natively in each language, varied in register and form
  (questions and orders, indirect requests, informal spellings, missing
  accents, typos), never translated from the other language and never taken
  from `evals/`.
- **Out-of-scope examples** are in `reflector/out-of-scope.toml` (`es = [...]`,
  `en = [...]`): what no built-in skill handles and the reflector answers or
  refuses. Requests to write text, open questions, opinions and advice,
  requests for skills not built yet (definitions, translation, the time
  somewhere else, weather, reminders), statements, near misses that share
  words with an intent ("el tiempo vuela", "time to go to bed"), the
  reflector's sentence shapes (docs/spec/reflector.md 2), acknowledgements and
  noise. The file is the out-of-scope class's whole definition: a phrase the
  classifier wrongly accepts is fixed by adding its *kind* here, in several
  forms, not the phrase alone.
- The **language model** learns from the same phrases, labelled by the
  language they are listed under.

Help lines (`help.es`, `help.en`) are never training data.

## 2. Features

`classifier::features::extract`, shared by the trainer and the engine, reads
the **match form** of a message (lower case, accents folded; DESIGN.md §4,
stage 0), so a missing accent changes nothing:

1. The text is reduced to words: letters are kept, and a run of three or more
   of one letter becomes one ("holaaa"); every run of digits becomes a single
   `0` ("2d6" and "3d8" are both `0d0`; numbers never become vocabulary);
   sentence punctuation (`¿?¡!.,;:`, quotes) separates words and is dropped;
   any other symbol (`+ * / ^ % = $ €`, an emoji) is a word of its own. At
   most the first 400 characters are read.
2. Features: the character n-grams of each word padded with a space at each
   end (`" hora "` gives `" h"`, `"ho"`, … ; n from `min_n` to `max_n`), never
   across words; each whole word; each pair of adjacent words.
3. A word the model does not know adds one to the **unknown-word** feature. In
   training, a word seen in fewer than two phrases also counts as unknown (and
   stays a feature), so the model learns what an unseen word says: unseen
   words are much more common in what is out of scope.

Each phrase is weighted by TF-IDF (`idf = ln((1 + N) / (1 + df)) + 1`) and
L2-normalised over the features the model knows.

| | intent model | language model |
|---|---|---|
| n-grams | 2 to 5 | 1 to 4 |
| words / word pairs | yes / yes | yes / no |
| unknown-word feature | yes | no |

## 3. Training

`cargo run --release -p talos-train` (a workspace member, never linked into
the `talos` binary; it shares the feature code and the file writer with the
engine) trains both models and writes them to `models/`. A few seconds.

- **Model**: multinomial logistic regression (softmax) over the features,
  fitted by stochastic gradient descent with L2 regularisation (3e-6), 40
  passes, learning rate 0.5 decaying as `1 / (1 + rate · l2 · step)`. Each
  class's examples weigh `(mean class size / class size)^0.5`, because
  out-of-scope is far larger than any intent. The language model has no
  biases, so a message with nothing it knows is a tie, never a decision.
- **Two fits**: the first chooses the features that keep a weight after
  pruning; the second fits on those alone, so the normalisation the engine
  computes (over known features only) is the one training used.
- **Pruning and quantisation**: a weight below 2 % of the largest is dropped;
  the rest are rounded to signed bytes in units of `largest / 127`. Features
  left with no weight leave the vocabulary.
- **Deterministic**: the examples are visited in an order drawn from a fixed
  seed (`SplitMix64`), arithmetic is `f64` in a fixed order, and `exp`/`ln`
  are Talos's own (`classifier::num`, basic operations only), so the same
  corpus gives byte-identical files on every machine.
  `cargo run --release -p talos-train -- --check` retrains and fails if the
  files differ.
- **Choosing parameters without the batteries**:
  `talos-train --cv 5` cross-validates on the training corpus itself (intent
  accuracy, wrong and missed intents, out-of-scope accepted, language
  decisions), and `--cv 5 --misses` lists the training phrases it got wrong.
  Flags (`--l2`, `--threshold`, `--margin`, `--prune`, `--balance`,
  `--min-n`, …) override the shipped parameters for experiments; the shipped
  ones are `Params::intent()` and `Params::language()` in `tools/train`.

## 4. Acceptance

The engine runs the intent model only when no rule matched (stage 4). It
ranks every class by probability and accepts the first only if:

1. it is an intent, not `out-of-scope`;
2. its probability is at least the **threshold** (0.6), or the
   **short-message threshold** (0.85) when the message has fewer than
   **short words** (2) words: one word carries little evidence, and the common
   single words are matched by rules anyway;
3. it leads the runner-up by at least the **margin** (0.3);
4. it is not an intent with side effects (`classifier::SIDE_EFFECTS`:
   `teach.assert`, `teach.forget`, `teach.phrase`, `preferences.set`,
   `preferences.forget`). Those need a rule match or a confirmation
   (DESIGN.md §11): a guess of one that passes 1–3 is offered to its handler
   with `Context::guessed` set, only to ask a yes/no that restates the
   change, and the engine applies no change made on a guess without that
   yes. When manifests can declare effects, the list moves there.

An accepted intent is dispatched like a rule's, with no captures. It carries
the slots its rules fix most often on its own examples, which the trainer
records per class (today `topic=intro` for `identity.who`, `topic=kind` for
`identity.nature`, `shape=of` for `calc.percent`), so a handler that relies on
a fixed slot can answer, and the slots its manifest's `guess` cues find in
the message, read by their extractors ("lanza tres dados" has `count=3`;
SKILLS.md §1). A handler that needs a capture answers with its
usage hint (`unhandled`, intent kept); one that *fails* without it sends the
turn on to the reflector instead, as if nothing had been recognised. The outcome's `confidence` is the accepted
probability (rules give 1.0). The thresholds live in the model file; they
were chosen by cross-validation (§3) and are re-chosen when the corpus grows.

The language model is asked only when the marker words decide nothing and the
conversation has not settled its language (ADR 0015); it decides at a
probability of 0.95 or more, and only for a message with a word of three
letters or more (a sum, a unit or "d20" is left to the conversation). A
classified language holds no more than a default: the next marker word
decides.

`talos explain` shows the top three classes, their probabilities and why the
first was or was not accepted:

```text
classifier clock.now 0.789, out-of-scope 0.165, identity.nature 0.011; accepted clock.now (0.789 ≥ 0.60, lead 0.624 ≥ 0.30)
```

## 5. How the model reaches the binary

The model files are **embedded** with `include_bytes!` (generated by
`crates/talos-skills/build.rs`, which embeds the manifests the same way) and
**read in place**: `Model` keeps the bytes and reads each value from them when
a turn needs it, copying nothing into the heap. The bytes are part of the
executable's read-only image, so the operating system pages them in only where
a turn touches them, like a memory-mapped file (ADR 0008), with no `unsafe` and
no file to install. A turn matched by a rule never touches them.

- Opening a model reads the header and checks that the sections fit the file;
  every later read is bounds-checked, so a damaged file can rank wrongly but
  never panic. The checksum and the order of the vocabulary are verified by
  `Model::verify`, run by the tests, the trainer and `talos skills check`,
  not at start-up (it would read every page).
- **Corpus hash.** The file records an FNV-1a hash of its training corpus
  (every intent's examples by skill id and manifest order, Spanish then
  English, then the out-of-scope corpus; comments and formatting do not
  count). The engine hashes its own manifests and the embedded
  `reflector/out-of-scope.toml` at start-up and **refuses** a model whose hash
  differs: it then runs on rules alone (as it does when `models/` is absent at
  build time), the classifier stage says why in the trace, and `talos skills
  check` warns. A test in `talos-skills` fails when the embedded models do not
  match, so a manifest change without retraining cannot pass CI.

Measured on an Apple-silicon Mac, release build: the intent model is 520 KB
and the language model 100 KB; the stage takes 20 µs at p50 and 32 µs at p99;
`talos ask "hola"` (a rule turn) stays at 7.0–7.7 MB maximum RSS; a chat whose
turns reach the classifier adds up to the models' size in clean, file-backed
pages (see DEVELOPMENT.md, Footprint).

## 6. File format

Version 1. Little-endian. Offsets in bytes.

| Offset | Size | Field |
|---|---|---|
| 0 | 8 | magic `TALOSCLF` |
| 8 | 2 | format version (1) |
| 10 | 1 | kind: 1 intent, 2 language |
| 11 | 1 | number of classes `C` (1–255) |
| 12 | 8 | corpus hash |
| 20 | 1 | `min_n` |
| 21 | 1 | `max_n` |
| 22 | 1 | whole-word features (0/1) |
| 23 | 1 | word-pair features (0/1) |
| 24 | 4 | number of features `V` |
| 28 | 4 | number of weights `P` |
| 32 | 4 | weight scale (f32): a weight is its byte times this |
| 36 | 4 | threshold (f32) |
| 40 | 4 | margin (f32) |
| 44 | 4 | short-message threshold (f32) |
| 48 | 1 | short words: messages with fewer words use the short threshold (0: none) |
| 49 | 7 | reserved, zero |

Then, in order:

1. **Classes**: `C` times: a length byte and the UTF-8 name; a byte with the
   number of slots a guess of the class carries; for each, a length byte and
   the name, a length byte and the value.
2. **Biases**: `C` f32.
3. **Vocabulary offsets**: `V + 1` u32, into the vocabulary bytes.
4. **Vocabulary bytes**: the features, sorted by byte value, distinct. A
   whole word is `\x01word`, a pair `\x02word word`, the unknown-word feature
   `\x03`; anything else is a character n-gram with its spaces.
5. **IDF**: `V` f32.
6. **Weight offsets**: `V + 1` u32, into the weights.
7. **Weights**: `P` pairs of (class index u8, quantised weight i8), grouped
   by feature, sorted by class within a feature.
8. **Checksum**: FNV-1a 64 of every byte before it.

A file whose version is not the one the binary reads is refused. Changing the
features or the layout means a new version and a retrain.
