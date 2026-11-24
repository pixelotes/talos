# Evals changelog

## Alignment with the final specification

The batteries predate the specification; labels and metadata were aligned
with it. Phrase text was not rewritten (independence rule), except where
stated. Counts are rows per file.

| File | Re-labelled | Moved to reject | Added / replaced | Other |
|---|---|---|---|---|
| `intents.es` | 3 | 4 | 7 added (4 replace moved rows, 3 restore `smalltalk.greet` to its minimum) | slots renamed or reformatted on 81 rows, dropped on 1 |
| `intents.en` | 3 | 4 | 4 added (replace moved rows) | slots renamed or reformatted on 82 rows, dropped on 1 |
| `reject.es` | 0 | 4 received | 0 | `status` on all 99 rows; `trace` on most; 3 re-kinded to `conversational` |
| `reject.en` | 0 | 4 received | 0 | same as es |
| `conversations.es` | 17 of 79 turns changed status or intent | 0 | 0 | `asks`, `ack`, `trace`, `given` added; 1 phrase replaced |
| `conversations.en` | 16 of 79 turns changed status or intent | 0 | 0 | same; 1 phrase replaced |

### Slots

Renamed to the spec's names, e.g. `x`/`y` to `subject`/`class` (`relations.is_a`),
`x` to `class`/`whole`/`part`/`topic` per relation, `min`/`max` to `low`/`high`
(`random.number`), bare `{"name": ...}` to `{"key": "name", "value": ...}`
(`preferences.set`), `{"place": ...}` to `{"key": "place"}`
(`preferences.forget`, which has no value). `direction` added to
`relations.causes` (it is required): English "what causes X" is `causes_of`,
"effects of X" `effects_of`, Spanish "qué provoca/causa/produce X" `both`. Dates
follow the format in the README. Dropped: the two phrases that state both a
name and a place (the spec has one `key` per turn).

### Re-labelled (intent changes)

- "Buenas tardes, ¿qué tal?" greet to how_are_you (greeting prefix is dropped); "yo, what's up" likewise.
- "buenas noches, Talos" greet to bye ("buenas noches" is always a farewell).
- "What day is it today?" clock.date to clock.weekday ("what day is it" is listed under weekday; Spanish "qué día es hoy" stays `clock.date`).
- "¿Qué clase de cosas son los mamíferos?" / "What kind of things are mammals?" relations.kinds to define.word ("what are Xs").

### Moved to reject (and replaced)

- "Soy de Bilbao, que no se te olvide" / "I'm from Bristol, don't forget": origin statements are not `preferences.set` (deflected, `self`).
- "Para que lo sepas…", "Que sepas que…" / "FYI spiders aren't insects", "For your information…" (two rows per language): not teaching cues, so ordinary statements (deflected, `assertion`).
- "¿Qué me depara el futuro?" / "what does the future hold for me?": predictions are not made, so not `fortune.tell` (unhandled).

### Reject rows

`status` added to every row (deflected / unhandled / answered), with `trace` where
the spec names one. Examples: "¿qué opinas de la guerra de Troya?" and "is it
better to rent or buy a flat?" are `deflected` (S16), not unhandled; "should I
study law or engineering?" is `unhandled · advice`; "pon una alarma" is
`unhandled · cannot`; "jajajajaja" and "hmmmm" are `conversational` and
`deflected`; "👍👍👍" is `answered`. New kind `conversational`.

### Conversations

- Turns where Talos asks back are `asks` with `slot` (they were `answered`):
  "¿cuánto falta?", "qué hora es en", "elige uno por mí", "recuerda mi nombre".
- "mejor no, olvídalo" / "actually, forget it" came after the question was
  already answered, so nothing was open: `ack`, not `cancel`.
- `random.number` has defaults and never asks: "del 1 al 20" is a new request,
  not a `follow_up`; "otro" / "another one" outside a fortune is `unhandled`.
- Deflected vs unhandled per the reflector: "¿tú qué opinas de la guerra de
  Troya?" is `deflected` (S16); "time flies" is `deflected` (S2); "vale, pizza
  entonces" and "cuento contigo" are `unhandled · catch-all`; "busca en internet…"
  is `unhandled · cannot`; "qué pereza todo" is `deflected · complaint`.
- Unsupported follow-up fragments ("¿y 20?", "por 5") are `unhandled`.
- "vivo en Valencia" / "I live in Valencia" in c25 replaced by Zaragoza / Leeds:
  Valencia is an ambiguous name that Talos must ask about (conversation.md 2.4),
  which would have changed the conversation's shape.

### Spec choices where files disagree or are silent

- "tradúceme este párrafo" stays in reject (generative, conversation.md 4.3) although knowledge.md 2.9 would keep `translate.word` for free translation.
- "Hola, ¿hay alguien ahí?" and similar stay `smalltalk.greet`: the specs do not say whether a presence check after a greeting is dropped like "¿qué tal?".
