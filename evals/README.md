# Evaluation batteries

Held-out test sets that gate Talos's recognisers in CI
([ADR 0008](../docs/adr/0008-intent-classification.md),
[ADR 0011](../docs/adr/0011-bilingual-by-construction.md)). Labels are the
intent ids of the catalogue in [`docs/spec/README.md`](../docs/spec/README.md);
`check.py` parses that table, so the catalogue is the single source of truth.

## Independence rule

These phrases are a **test set**. They must never be copied, paraphrased
closely, or used as training examples in skill manifests, rule patterns,
reflector scripts or the correction table. Manifest authors should not read or
mine this directory; write examples from the specification instead. A test
set that has leaked into training measures nothing. If a battery phrase is
found in a manifest, remove it from the manifest, not from the battery.

The two languages are authored natively, not as translations of each other,
and are not parallel: counts and phrasing differ on purpose.

## Files

| File | Purpose |
|---|---|
| `intents.{es,en}.jsonl` | Phrases with the intent they must be recognised as (at least 8 per catalogue intent per language). |
| `reject.{es,en}.jsonl` | Messages that must be recognised as **no** intent, with the status Talos must give them. |
| `conversations.{es,en}.jsonl` | Multi-turn stand-alone conversations: pending dialog, cancellation, reflector, unhandled. |
| `check.py` | Dependency-free validator. `python3 evals/check.py` prints a summary and exits non-zero on any problem. |
| `CHANGELOG.md` | What was aligned with the specification, and how many rows each change touched. |

## Where the labels come from

The labels follow the behavioural specification in [`docs/spec/`](../docs/spec/README.md):
the intent catalogue and the section "Which status an outcome gets" in
`README.md`; the ownership of ambiguous phrasings in each intent's
**Does not** list and in [`decisions.md`](../docs/spec/decisions.md); slot
names and types in each intent's **Slots** line; dialog behaviour in
[`conversation.md`](../docs/spec/conversation.md); and the `deflected` /
`unhandled` boundary in [`reflector.md`](../docs/spec/reflector.md). Where
two spec files disagree, the choice made is listed in `CHANGELOG.md`. The
phrases themselves are not taken from the specification (independence rule).

## Formats

One JSON object per line, UTF-8, ids unique across all files.

**intents**: `{"id": "es-0001", "text": "...", "lang": "es", "intent": "clock.now", "slots": {...}, "tags": [...]}`

- `slots` (optional): expected extracted slots, only for rows where the
  phrase states them. Names are exactly those of the intent's **Slots** line
  in the specification; `check.py` rejects any other name (its `SLOTS` table
  mirrors the spec). A harness may ignore slots or compare only the ones a
  skill declares. Value formats:
  - `number`: a JSON number (`"count": 2`); `term`, `choice`: a string as
    typed (choices by their spec value: `metric`/`imperial`, `name`/`place`/`units`,
    `causes_of`/`effects_of`/`both`);
  - `place`: a string as typed (a list if several are named);
  - `language`: `es` or `en`;
  - `term` list (`options`): a list of strings;
  - `datetime`: ISO 8601 date (`2000-01-01`) when the phrase fixes it;
    `--MM-DD` when the year is not given (the spec resolves it to the next
    occurrence relative to `now`, which a battery cannot know); `+N d` /
    `-N d` for a day relative to today (`+1 d` is tomorrow).
  - A phrase that carries two values for a slot the spec defines once (a name
    *and* a place for `preferences.set`) has no `slots`.
- `tags` (optional): `typo`, `informal`, `no-accents`, `long`, `short`,
  `indirect` (the request is implied rather than stated). Used to slice
  accuracy.
- The label is the intent the phrase is *recognised* as. Whether Talos can
  then fulfil it is a separate matter: a recognised intent that cannot be
  fulfilled (an unknown word, a weekend as a date) ends `unhandled` with the
  intent kept, and is still labelled with the intent here.

**reject**: `{"id": "es-r001", "text": "...", "lang": "es", "intent": null, "kind": "near_miss", "status": "deflected", "trace": "assertion"}`

- `kind` is one of `generative`, `open_question`, `statement`, `future_skill`,
  `near_miss`, `noise`, `conversational`. Near-misses look like an intent but
  are not ("time flies", "cuento contigo"). `conversational` is for acts that
  are not intents: laughter, backchannels, acknowledgements.
- `status` (required): the outcome the message must get although it has no
  intent: `deflected` (a recognised reflector shape), `unhandled` (a guard, a
  refusal or the catch-all) or `answered` (only a bare acknowledgement such as
  a thumbs-up, conversation.md 1.2 and 4.2).
- `trace` (optional): the reflector shape (for `deflected`) or the reason (for
  `unhandled`), as the specification names them in `reflector.md`. It is
  informative: a harness may report it but should not gate on it. It is
  omitted where two shapes are equally defensible and the status is the same.

**conversations**: `{"id": "es-c01", "lang": "es", "given": "...", "turns": [{"user": "...", "expect": {"status": "answered|asks|deflected|unhandled", "intent": "<id or null>", ...}}]}`

- `given` (optional, per conversation): knowledge the turns depend on (a
  word, a class, a part-of fact). The statuses of those turns assume it is in
  the store; a harness that cannot provide it should skip the conversation.
  Everything else assumes an empty user store and no stored preferences. Each
  conversation starts a fresh session.
- `status` is the outcome of the turn:
  - `answered`: a complete answer, including a correct "no" and an honest "I
    don't know" about the user's own store;
  - `asks`: Talos asks back for a missing or ambiguous slot. Not a final
    outcome (README, "Which status an outcome gets"). `intent` is the
    intent being completed and `slot` names what is asked for;
  - `deflected`: only the reflector, on a recognised shape; `intent` is null;
  - `unhandled`: Talos could not provide what was asked. `intent` is the
    recognised intent when there was one (`unhandled · define.word`), null
    when nothing was recognised (reflector guards and catch-all, bare
    "sí"/"no" with nothing open, generative requests, capabilities Talos lacks).
- `follow_up: true` marks the turn that answers the preceding `asks` turn (the
  expectation sees the message first, conversation.md 3.2); status `answered`
  and the same intent. Any other message after an `asks` releases and closes
  the question and is judged as if nothing was open (3.6).
- `cancel: true` marks a cancellation word said while a question is open:
  `answered`, intent null, the original request is not run (3.5).
- `ack: true` marks a minimal acknowledgement with nothing to act on: `answered`,
  intent null. It covers "vale"/"ok", and cancellation words said with
  nothing open ("olvídalo": 3.5).
- `trace` (optional): as in reject rows, for `deflected` and `unhandled` turns
  without an intent.
- Only follow-ups the specification names are supported (a bare place after a
  time or difference question, a new term after a translation, topic
  follow-ups in `knowledge.md`). Other fragments ("y 20" after a conversion,
  "por 5" after a sum) and a bare "otro" outside a fortune are not
  understood: `unhandled`, no intent, reason `catch-all`.

## Independence of the labels from a store

Knowledge turns (`define`, `relations`, `teach`, `translate`) that depend on
the contents of the store say so in `given`. Intent rows are labelled by what
the phrase *is*, never by whether the store happens to know the word.

## Metrics

- **Intent accuracy per language**: share of `intents.<lang>` phrases whose
  top accepted intent equals the label, overall, per intent and per tag. CI
  floor per language plus a maximum gap between languages (ADR 0011).
- **False-accept rate** on the rejection set: share of `reject.<lang>`
  messages that any stage (rules or classifier) accepts as an intent. Report
  per `kind`; `near_miss` is the number to watch.
- **Missed-accept rate**: intent phrases rejected (below threshold or margin),
  reported separately from confusions so thresholds can be tuned.
- **Catch-all rate** on conversations: share of turns ending `unhandled` that
  the file did not expect to be unhandled, and overall share of turns ending
  in the bare catch-all (ADR 0009).
- **Conversation pass rate**: conversations where every turn matches status
  and intent (and `slot` for `asks` turns).
- **Status accuracy on the rejection set**: share of `reject.<lang>` messages
  whose status (`deflected` / `unhandled` / `answered`) matches, in addition to
  the false-accept rate.

`talos eval` (`--json` for the machine form) reports these per language twice,
with rules only and with rules plus the classifier: intent accuracy over all
rows and over the rows of built skills, wrong intents apart from missed ones,
false accepts by kind, status accuracy on the rejection set, the conversation
battery's catch-all and unexpected-unhandled rates, and accuracy per intent.
It prints counts only, never a phrase. Tags and conversation pass rate are not
reported yet.

Do not tune thresholds on one file and report on the same file only; keep a
record of what was tuned against, and prefer reporting after any tuning on
fresh additions.

## Adding phrases

Add to the end, with the next id, and run `check.py`. Keep at least 8 phrases
per intent per language, no exact duplicates across files. Prefer phrases
that real users could plausibly type, including ones the current rules get
wrong.
