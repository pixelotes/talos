# Skills

A skill is one capability: telling the time, doing arithmetic, defining a
word, playing chess. It has two parts:

- a **manifest** — data: what the skill understands and how it speaks;
- a **handler** — code: what it does with a recognised request.

Decisions: [ADR 0007](adr/0007-declarative-skill-manifests.md),
[ADR 0009](adr/0009-reflector-fallback.md),
[ADR 0013](adr/0013-external-skills.md).

## 1. Manifest

One TOML file per skill. It is the single source for rules, classifier
training examples, slot definitions, replies and the help text.

```toml
[skill]
id = "clock"
summary.en = "the current time and date"
summary.es = "la hora y la fecha"
names.en   = ["clock", "time"]   # what a user calls it, for help.skill
names.es   = ["reloj", "hora"]
help.en    = ["what time is it"]    # shown by help; never training examples
help.es    = ["qué hora es"]
capabilities = []                # e.g. ["network"]

[[intent]]
id = "clock.now"
priority = 50                    # explicit; higher wins among rule matches
courtesy = false                 # true for greetings and thanks (see below)
command = { es = "/hora", en = "/time" }   # optional shortcut (see below)

  [intent.examples]              # classifier training data
  en = ["what time is it", "do you have the time", "tell me the time"]
  es = ["qué hora es", "me dices la hora", "qué hora tienes"]

  [[intent.pattern]]             # optional certain matches
  lang  = "es"
  regex = '^que hora (es|tienes)\b'     # lower case, no accents

  [[intent.slot]]
  name     = "date"
  type     = "date"              # term | number | date | duration | datetime |
                                 # daterange | place | language | choice | confirm
  required = false
  repeated = false               # only for `term`: a list of options
  internal = false               # true: steers the handler, not reported

[reply.now]
en = ["It's {time}.", "{time}."]
es = ["Son las {time}.", "Las {time}."]

[reply.now_in]
en = ["In {place} it's {time}."]
es = ["En {place} son las {time}."]
```

Rules that follow from the format:

- **Both languages or neither.** A manifest with a summary, examples, names,
  help or reply in only one language fails validation. (Explicitly marking an
  intent single-language, when that is genuinely the case, is not built yet.)
- **Unknown keys are errors**, so a typo cannot pass silently.
- **Examples are training data.** The classifier ([MODEL.md](MODEL.md)) learns
  from every intent's examples, together with the out-of-scope corpus in
  `reflector/out-of-scope.toml`. After changing either, retrain with
  `cargo run --release -p talos-train`: the engine refuses a model trained on
  other examples and runs on rules alone.
- **Patterns match the match form** of the message: lower case, accents and
  diacritics removed (`¿Qué día es MAÑANA?` is matched as
  `¿que dia es manana?`). Write them lower-case and unaccented; `talos skills
  check` rejects a pattern that could never match. A capture still reaches the
  handler in the user's own spelling. See
  [SKILL_AUTHORING.md](SKILL_AUTHORING.md) §2.1.
- **`courtesy = true`** marks an intent whose phrasing may open or close a
  message that carries a request. When no rule matches the whole message, a
  courtesy of up to four words at either end is dropped if the rest matches.
- **`internal = true` slots** are set by fixed values in patterns
  (`slots = { register = "casual" }`) and tell the handler which phrasing was
  used; they are left out of the outcome.
- **Priority is a number in the file**, never a position in source code.
- **Replies are templates with named variables.** Handlers never build
  sentences.
- **The help answer is generated** from `summary`, so "what can you do?"
  cannot drift from what is installed.

- **Commands** ([ADR 0016](adr/0016-command-shortcuts.md)). An intent may
  declare a shortcut: `command = { es = "/calc", en = "/calc", slot =
  "expression" }`. Each name is `/` and lower-case ASCII (letters, digits,
  `_`); both languages' names always work, and the reply is in the
  conversation's language. `slot`, if given, is a slot the intent
  declares (an internal one: free text the handler reads, below): the rest
  of the line fills it, read by that slot type's
  extractor exactly as a rule capture is. A command without `slot` takes no
  argument, and one given anyway is refused (`unhandled ·
  command-argument`), never silently ignored. A command is the first check
  of the rule stage: no rules, classifier, safety check or reflector run on
  it, and it closes any open question. A missing argument is a missing slot,
  so the handler asks for it. An unknown command is `unhandled ·
  unknown-command`, and the reply says how to list them: `help` shows each
  skill's commands after its summary. An intent that needs several
  independent slots either takes one free-text argument its handler reads,
  or declares no command.
- **`choice` slots** list their options, each with its id and its names in
  both languages: `options = [{ id = "metric", es = ["métrico"], en =
  ["metric"] }, …]`. The slot holds the chosen id.
- **`guess`** on a public slot says where its value sits in a message the
  classifier recognised, which has no captures: `guess = { before =
  ["dados", "dice"] }` (the one to three words before a cue: "tres" in
  "lanza tres dados"), `guess = { after = ["entre", "between"] }` (everything
  after a cue to the end, closing courtesies dropped), `guess = { words = {
  name = ["nombre", "name"] } }` (the value whose words occur, when only one
  value's do). Cues are in either language and may be several words. The
  value is read by the slot's extractor; one that cannot be read, or two
  readings that differ, give nothing, and the handler asks as after a rule.
- **A command's slot** may be internal: it then takes the rest of the line
  as free text that the handler reads and reports as the sentence form's
  slots (`/dado 2d6` fills `notation`; the outcome is `count=2 sides=6`).

`talos skills check` validates every manifest together with its handler.
Errors (exit status 1): the manifest does not parse (schema, a missing
language, a bad template or slot type, a command name that is not `/` and
lower-case ASCII, a command slot the intent does not declare, a `guess` on
an internal slot or with no cue words); the id differs from the directory; a
manifest without a handler or the reverse; `names` or `help` missing in a
language; a reply id the handler declares that the manifest lacks, or a
manifest reply no handler declares; a pattern that does not compile, captures
an undeclared slot or is written with accents or capitals; two intents of equal
priority whose rules in one language match the same example; one command name
declared by two intents. Warnings: an
example taken by a rule of an intent with higher priority; an intent with no
pattern (only the classifier can reach it); variants of one reply that
use different variables. It also reports the classifier models, and warns when
one is missing or was trained on other manifests.

## 2. Handler

```rust
pub trait Skill: Send + Sync {
    fn id(&self) -> &'static str;                 // the manifest's id
    fn replies(&self) -> &'static [&'static str]; // every reply id it can return
    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult;
}

pub struct Call<'a> {
    pub intent: &'a IntentId,   // always one of the skill's own
    pub lang: Lang,             // the turn's language, already decided
    pub slots: &'a Slots,       // typed, already extracted; internal ones included
    pub text: &'a str,          // as received; never for deciding the intent
}

pub enum SkillResult {
    Answer(Message),               // template id + variables; status Answered
    Ask(Expectation, Message),     // a question back to the user
    Unable(Message),               // recognised, cannot provide: Unhandled, intent kept
    Decline,                       // not mine; try the next candidate
    Fail(SkillError),              // an honest error; Unhandled
}
```

`Context` gives a handler what it may use and nothing else: the injected
clock (`now()`), the caller's place (`place()`), the turn's random source
(`random()`, one draw per call, seeded so that turns replay), this
conversation's state (`conversation()`: turn number, language, recent topic),
the user's preferences (`preferences()`: name, place, units; the remembered
place is the reference place when a message names none, docs/spec/state.md),
the installed manifests (`skills()`, for generated answers such as help),
`report()` to put a slot on the outcome (a default the handler applied),
`guessed()` (whether the classifier, not a rule, recognised the request) and
`remember()` to ask the engine to store or forget a preference, applied only
if the handler answers. A handler that changes something asks the user to
confirm before acting on a guess; the engine drops a change made on a guess
without a `confirm` slot answered yes. A handler reads no system clock,
environment, file or global state. The knowledge store, and an HTTP client
when the `network` capability is granted, join the context as they are built.

`Ask(expectation, message)` asks the user one short question (the message)
and says what answers it (the expectation, §3). The turn is `Answered`, the
outcome's `pending` is true, and its line in a transcript is `asks · intent ·
slot`. When the answer comes, the engine calls the same handler again with
the request's slots and the answered one filled in; the handler never sees
the dialog. A skill asks for a required slot that is missing, and for a value
that is ambiguous (English `3/4`); never for an optional one. How to write a
built-in skill, step by step, is in [SKILL_AUTHORING.md](SKILL_AUTHORING.md).

Handlers are synchronous. One that uses the network must honour the timeout
it is given and degrade to a clear "I can't reach that right now" message.

## 3. Slots and expectations

Slot types are shared and typed, and each extractor is bilingual and tested
independently of any skill (`talos_core::extract`):

| Type | Reads | Value |
|---|---|---|
| `term` | a quoted or trailing phrase, in the user's spelling; repeatable as a list of options | `Text`, `List` |
| `number` | exact decimals; digits and words; separators by the turn's language | `Number` |
| `date` | a day, absolute or relative to `now`; English `3/4` is ambiguous | `Date` |
| `duration` | "hora y media", "45 min", "1h30", "a fortnight"; a vague quantity is not one | `Duration` (calendar months apart from seconds) |
| `datetime` | a day, a time of day or both: "mañana por la tarde", "next Friday at 9 pm" | `Date` or `DateTime` |
| `daterange` | "esta semana", "next month", "el finde" (weeks run Monday to Sunday) | `DateRange` |
| `place` | a name kept as typed (no gazetteer yet), a zone or offset ("UTC+2", "CET"), or a reference: "aquí"/"here", "mi ciudad"/"home" (the place in `cx.preferences()`), "allí"/"there"; "somewhere warm" is not one | `Place` |
| `language` | "inglés", "in Spanish", "al español"; other languages by code for the skill to refuse | `Language` |
| `choice` | one of the options the manifest lists, by position ("la segunda", "the last one") or by name | `Choice` (the option's id) |
| `confirm` | yes or no (conversation.md 3.3) | `Confirm` |

A capture that cannot be read becomes an `Invalid` value, never a guess.

An `Expectation` names the slot the answer fills and what it accepts:
`Expectation::slot` (a value of the slot's type), `completing` (text that
continues what the user wrote: "2 +", then "3"), `choice` (one of a few
options, each with the text that fills the slot and its names in both
languages) or `confirm` (yes or no). The engine keeps it in the conversation
(one at a time) with the request's slots, and offers the next message to it
first (docs/spec/conversation.md §3):

- an answer, bare, framed ("pues Tokio", "London, thanks") or inside the
  request repeated, completes the original request, in the language of the
  answering turn;
- "déjalo", "never mind" and the like cancel it (`answered · cancelled`);
- a hesitation ("a ver", "hold on") keeps it open (`answered · hedge`);
- a message a rule recognises as another request releases it: it is
  answered normally and the question is closed;
- anything else is a failed answer: the question is asked once more, then
  dropped (`unhandled · gave-up`, intent kept);
- it expires three turns, or five minutes by the turns' clock, after it was
  asked; a distress message or a command closes it.

With nothing open, a bare "sí"/"no" is `unhandled · nothing-pending` and an
acknowledgement ("vale", "ok", "olvídalo") is `answered · acknowledgement`.

## 4. Built-in skills

Planned for the first major version; built so far: `smalltalk`, `identity`,
`help`, `clock`, `timezones`, `calc`, `random`, `fortune` and `preferences`.
Network skills are disabled until enabled in configuration.

| Skill | Does | Network |
|---|---|---|
| `smalltalk` | greetings, thanks, goodbyes | no |
| `identity` | who Talos is, what it runs on | no |
| `help` | what it can do, generated from manifests | no |
| `clock` | time, date, weekday, "how long until" | no |
| `timezones` | the time in another place, the time difference between two | no |
| `calc` | arithmetic, percentages, unit conversion | no |
| `random` | dice, coin, pick one, random number | no |
| `fortune` | a saying from a bundled list | no |
| `define` | what a word means; sense disambiguation | no |
| `relations` | is-a, part-of, kinds-of, cause queries | no |
| `teach` | teach, correct, forget, list what was taught | no |
| `translate` | words and stock phrases between es and en | no |
| `preferences` | remember the user's name, place, units | no |
| `games` | chess; small games (hangman, guess the number) | no |
| `weather` | current and forecast for a place | yes |
| `search` | a short factual summary from a configured source | yes |
| `news` | today's headlines from a configured source | yes |
| `wikipedia` | an article's opening and an extractive summary, with its link | yes |

Notes:

- **Translation** is dictionary-level by design: lexemes sharing a concept,
  plus a table of stock phrases. Translating arbitrary sentences is a language
  model's job and Talos says so.
- **Search** returns a sourced extract, never a synthesis.
- **Chess** keeps its game state in the conversation and uses a permissively
  licensed rules library with a small search of its own; an external UCI
  engine can be configured for stronger play.
- **Timers, alarms and reminders** require Talos to speak first, which needs
  a scheduler and a way to reach the user; they are deferred until after 1.0
  ([ROADMAP.md](ROADMAP.md)).
- **Translation of whole sentences** stays out: `translate` covers words and
  a table of stock phrases, and anything else is `unhandled` for a host to
  pass on.

## 5. External skills

Deferred until after 1.0 ([ADR 0013](adr/0013-external-skills.md), amended):
Talos stands on its own and every planned skill is built in. What follows is
the design kept for later. The manifest parser does not accept `[handler]` or
`generated` today.

A manifest may point at a program or a URL instead of a compiled handler:

```toml
[skill]
id = "homelab"
capabilities = ["exec"]

[handler]
command = ["/usr/local/bin/homelab-skill"]   # or: url = "http://127.0.0.1:9000/skill"
timeout_ms = 1500
```

> **Transport not final.** What follows is Talos's own minimal JSON protocol.
> The Model Context Protocol is the other candidate — and the likely one if
> skills are to be shared with a host such as Tomos, with both programs using
> the same MCP servers. The manifest is the same either way; only the
> `[handler]` table and the wire format differ. Decided in v0.5
> ([ADR 0013](adr/0013-external-skills.md)).

Protocol, one JSON object each way:

```json
{ "intent": "homelab.status", "slots": {"host": "nas"},
  "lang": "es", "text": "cómo está el nas", "conversation": "c-42" }
```

```json
{ "status": "answer", "reply": { "id": "status", "vars": { "host": "nas", "state": "ok" } } }
```

`status` is `answer`, `ask`, `decline` or `error`. The reply is still a
template id and variables from the skill's own manifest, so external skills
are bilingual and deterministic in form like built-in ones. External skills
are an explicit allow-list in configuration.

This is how capabilities are added without recompiling Talos. It is not an
integration with any particular host: Talos and a host such as Tomos share no
skills directly.

### Skills that use a model

No built-in skill uses a language model, and none ever will
([ADR 0012](adr/0012-no-language-model.md)). An external skill may — for
instance an ephemeral specialist that starts, extracts structure from a text,
and exits. Such a skill must say so:

```toml
[skill]
id = "extract"
capabilities = ["exec"]
generated = true
```

Every outcome it produces is marked `generated: true`. Talos does not start,
stop or locate models; the skill owns all of that.

## 6. The reflector

The fallback when no skill answers. It is an ELIZA-style script: ranked
keywords, decomposition patterns, reassembly templates and pronoun reflection,
one script per language, stored as data in `reflector/`.

Design rules:

- **Recognise the form of the sentence, never pretend to understand its
  content.** A good pattern hands the conversation back; a bad one sounds like
  comprehension.
- **More sentence shapes, not more therapy.** Coverage means recognising
  statements, opinions, plans, complaints and questions about Talos itself —
  not asking how the user feels.
- **Questions that look factual are not reflected.** "What is the capital of
  Peru?" with no answer in the store is `Unhandled` with an honest reply, not
  a deflection.
- **Status is explicit.** A match on a real pattern is `Deflected`; the
  catch-all is `Unhandled`, and so are a few recognised shapes Talos cannot
  help with (requests for advice, commands it cannot carry out). Hosts that have something better behind Talos use
  that to pass the turn on.

The measure of the reflector is the share of turns, in a stand-alone
evaluation battery, that end in the bare catch-all.
