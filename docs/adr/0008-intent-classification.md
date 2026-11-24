# 0008. Rules first, an offline-trained linear classifier second

- Status: Accepted
- Date: 2026-11-24

## Context

Recognising what the user wants must be fast, small, deterministic, bilingual
and able to say "none of the above". Pure rules are precise but brittle to
paraphrase. A machine-learning library at run time breaks the memory budget.

Prior measurements on the existing corpus showed character n-gram features
with a linear model handling Spanish and English morphology without a stemmer,
and showed Spanish lagging English, which points at corpus coverage more than
at model capacity.

## Decision

Two recognisers in sequence:

1. **Rules.** Patterns from manifests, compiled per language, ordered by
   explicit priority. A match is certain. A small user-correction table
   (normalised phrase → intent) is consulted at the same stage.
2. **Classifier.** Character n-gram TF-IDF features into a multinomial linear
   model.
   - Trained **offline** with a development-time tool from manifest examples.
   - Exported as a compact, versioned binary: a vocabulary table and pruned,
     quantised weights. Target size under 2 MB; memory-mapped.
   - **Inference is implemented in Rust** as a sparse dot product. No ML
     dependency at run time.
   - A prediction is accepted only above an absolute threshold and with a
     margin over the runner-up. Out-of-scope is a trained class.
   - The model file records a hash of the corpus it was trained on; the
     binary refuses a model that does not match its manifests.

Quality is gated by held-out evaluation batteries per language, with a
rejection set, that are never derivable from the training examples.

## Consequences

- Run-time cost is microseconds and a couple of megabytes.
- The classifier never dispatches a side-effecting action by itself; such
  intents require a rule match or a confirmation.
- Changing manifests requires retraining, which must be one command and part
  of the build.
- Users cannot retrain on the device. The correction table covers the common
  case ("when I say this, I mean that") without it.
- Thresholds are tuned against the batteries and need re-tuning when the
  corpus grows.

## Alternatives considered

- **Rules only.** Too brittle; every paraphrase becomes a pattern.
- **Small transformer or sentence embeddings.** Better generalisation, tens
  to hundreds of megabytes and a model runtime. Against the project's purpose.
- **Training in Rust at start-up.** Removes the Python tool, costs start-up
  time and complexity. Worth revisiting once the format is stable.
- **Feature hashing instead of a vocabulary.** Simpler, but a dense weight
  matrix large enough to avoid collisions is bigger than a pruned sparse one.
