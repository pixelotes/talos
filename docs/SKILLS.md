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

  [intent.examples]              # classifier training data
  en = ["what time is it", "do you have the time", "tell me the time"]
  es = ["qué hora es", "me dices la hora", "qué hora tienes"]

  [[intent.pattern]]             # optional certain matches
  lang  = "es"
  regex = '^que hora (es|tienes)\b'     # lower case, no accents

  [[intent.slot]]
  name     = "date"
  type     = "date"              # today: term | number | date
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

`talos skills check` validates every manifest together with its handler.
Errors (exit status 1): the manifest does not parse (schema, a missing
language, a bad template or slot type); the id differs from the directory; a
manifest without a handler or the reverse; `names` or `help` missing in a
language; a reply id the handler declares that the manifest lacks, or a
manifest reply no handler declares; a pattern that does not compile, captures
an undeclared slot or is written with accents or capitals; two intents of equal
priority whose rules in one language match the same example. Warnings: an
example taken by a rule of an intent with higher priority; an intent with no
pattern (unreachable until the classifier exists); variants of one reply that
use different variables.

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
the user's preferences (`preferences()`; empty until v0.3), the installed
manifests (`skills()`, for generated answers such as help) and `report()` to
put a slot on the outcome (a default the handler applied). A handler reads no
system clock, environment, file or global state. The knowledge store, and an
HTTP client when the `network` capability is granted, join the context as they
are built.

`Ask` is declared but not available until v0.2: today the engine renders such
a turn as `Unhandled` with reason `cannot-ask`, and a skill whose required
slot is missing answers `Unable` with a usage hint instead. How to write a
built-in skill, step by step, is in [SKILL_AUTHORING.md](SKILL_AUTHORING.md).

Handlers are synchronous. One that uses the network must honour the timeout
it is given and degrade to a clear "I can't reach that right now" message.

## 3. Slots and expectations

Slot types are shared and typed. Available today: `term` (a quoted or
trailing phrase, in the user's spelling), `number` (exact decimal; digits and
words) and `date` (absolute or relative to the turn's `now`); a slot that
cannot be read becomes an `Invalid` value, never a guess. Planned: `duration`,
`datetime`, `daterange` ("this week"), `place`, `language`, `choice`,
`confirm` (yes or no). A `term` slot may be declared repeatable (a list of
options). Extractors are bilingual and tested independently of any skill.

From v0.2, when a required slot is missing, the skill returns `Ask`. The expectation
records which skill asked, which slot is open and when it expires. The next
message in that conversation is offered to it first. "Never mind" / "déjalo"
cancels; an unrelated message releases the expectation and is routed normally.

## 4. Built-in skills

Planned for the first major version; of these only `smalltalk` is built so far
(its `smalltalk.greet` intent). Network skills are disabled until enabled in
configuration.

| Skill | Does | Network |
|---|---|---|
| `smalltalk` | greetings, thanks, goodbyes | no |
| `identity` | who Talos is, what it runs on | no |
| `help` | what it can do, generated from manifests | no |
| `clock` | time, date, weekday, "how long until" | no |
| `timezones` | the time in another place | no |
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

Notes:

- **Translation** is dictionary-level by design: lexemes sharing a concept,
  plus a table of stock phrases. Translating arbitrary sentences is a language
  model's job and Talos says so.
- **Search** returns a sourced extract, never a synthesis.
- **Chess** keeps its game state in the conversation and uses a permissively
  licensed rules library with a small search of its own; an external UCI
  engine can be configured for stronger play.
- **Timers and reminders** are not listed: they require Talos to speak first.
  See the open questions in [DESIGN.md](DESIGN.md).

## 5. External skills

Not built yet: the manifest parser does not accept `[handler]` or
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
