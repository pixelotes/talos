# Writing a built-in skill

How to add a skill to Talos. Read [SKILLS.md](SKILLS.md) for the idea, your
skill's section of [spec/basics.md](spec/basics.md) for the behaviour, and
[spec/conversation.md](spec/conversation.md) for what every skill inherits
(language, numbers, dates, determinism). This guide is the contract; the
worked example is `smalltalk.greet`.

## 1. What a skill owns

A skill `<id>` (for example `clock`) is exactly these files, and nobody else
edits them:

| File | What |
|---|---|
| `skills/<id>/manifest.toml` | intents, examples, patterns, slots, replies, help text |
| `crates/talos-skills/src/<id>.rs` | the handler |
| `golden/<id>/*.txt` | golden transcripts, one file per intent or per theme |

And it adds **one shared line**: in `crates/talos-skills/src/lib.rs`, the
`skills!` list already names every v0.1 skill, commented out, in catalogue
order. Uncomment yours:

```rust
skills! {
    smalltalk::Smalltalk,
    // identity::Identity,
    clock::Clock,            // ← your line, uncommented
    ...
}
```

That line declares the module `<id>` (the file `src/<id>.rs`) and registers
the handler, a unit struct with the name written there
(`pub(crate) struct Clock;`). The list's order is
the order `help` lists skills in; do not reorder it.

A skill does **not** edit `talos-core`, the binary, other skills, `Cargo.toml`
or the docs. If you need something the engine does not offer (an extractor,
a slot type, a `Context` accessor), stop and ask the integrator; do not work
around it in your skill. A skill adds no dependency.

The manifest and the handler go together: the build embeds every
`skills/*/manifest.toml`, and a manifest without its registered handler (or
the reverse) stops the engine from building.

## 2. The manifest

```toml
[skill]
id = "clock"                                  # = the directory name
summary.es = "la hora y la fecha"             # one line, for help.capabilities
summary.en = "the time and date"
names.es = ["reloj", "hora"]                  # what users call it, for help.skill
names.en = ["clock", "time"]
help.es = ["qué hora es", "qué día es hoy"]   # example requests shown by help
help.en = ["what time is it", "what's the date"]
capabilities = []                             # built-in v0.1 skills need none

[[intent]]
id = "clock.weekday"                          # from docs/spec/README.md's catalogue
priority = 50                                 # higher wins among rule matches
courtesy = false                              # see §2.4; only greetings and thanks
command = { es = "/diasemana", en = "/weekday", slot = "date" }  # optional, §2.5

  [intent.examples]                           # the classifier's training data
  es = ["qué día de la semana es", "qué día cae el 3 de mayo"]
  en = ["what day of the week is it", "what day is May 3"]

  [[intent.slot]]
  name = "date"
  type = "date"                               # see SKILLS.md §3 for every type
  required = false                            # true: ask for it when missing
  repeated = false                            # only for term: a list of options
  internal = false                            # true: steers the handler only

  [[intent.pattern]]
  lang = "es"
  regex = '^que dia (?:de la semana )?(?:es|cae|sera)(?: (?<date>.+?))?[\s?.!]*$'

  [[intent.pattern]]
  lang = "en"
  regex = '^what day (?:of the week )?is (?<date>.+?)[\s?.!]*$'
  slots = { }                                 # fixed values for declared term slots

[reply.weekday]
es = ["El {date} es {weekday}."]
en = ["{date} is a {weekday}."]
```

Unknown keys are errors. Every summary, example list, name list, help list
and reply needs both `es` and `en`, written natively in each language, never
translated.

### 2.1 Patterns match the match form

Before the rules run, the message is normalised: Unicode NFKC, lower case,
curly quotes and dashes made plain, `×` → `*`, `÷` → `/`, whitespace
collapsed and trimmed. Rules then see the **match form**, which also drops
diacritics from Latin letters: `¿Qué día es MAÑANA?` is matched as
`¿que dia es manana?`.

So write patterns **in lower case and without accents**: `que`, `dia`,
`manana`, `cuanto`. `talos skills check` rejects a pattern with an accented
letter or a capital outside an escape (`\S`, `\D` are fine). `¿`, `¡`, `ñ`→`n`
and `ü`→`u` follow the same rule: `ñ` never appears in a pattern.

- Patterns are `regex` crate syntax: linear time, no look-around, no
  back-references.
- Anchor them (`^…$`) and allow the usual edges: leading `¿¡`, trailing
  `[\s?.!]*`. Keep to the obvious phrasings; paraphrase is the classifier's
  job (docs/MODEL.md).
- Examples are what the classifier learns from: give each intent dozens per
  language, written natively (questions and orders, indirect requests,
  informal spellings, missing accents), never translated and never taken
  from `evals/`. After changing any, retrain with
  `cargo run --release -p talos-train`; a test fails until the models match
  the manifests. Phrases that look like your intent but are not belong in
  `reflector/out-of-scope.toml`.
- The turn's language rules are tried first, then the other language's, so
  a Spanish pattern still catches "qué hora es" in an English turn.
- Among matches, higher `priority` wins; ties go by intent id, then pattern
  order. Two intents of **equal priority** whose rules match the same
  example are an error in `talos skills check`; an example of yours taken by
  another intent with higher priority is a warning. Leave priority 100 to
  `smalltalk`.

### 2.2 Slots

Every named capture `(?<name>…)` must be a declared slot, and every fixed
value in a pattern's `slots = { … }` must be a declared `term` slot.
Captures are turned into typed values before your handler runs:

| `type` | Extractor | Value |
|---|---|---|
| `term` | the capture in the user's **original** spelling, case and accents, edge punctuation trimmed | `Value::Text` |
| `term`, `repeated = true` | split at commas, `/`, "y"/"e"/"o"/"u"/"and"/"or"; repeats removed | `Value::List { items, uncertain }`; `uncertain` when "y" and "o" were mixed |
| `number` | digits by the turn's language conventions (`1.500` is 1500 in Spanish, 1.5 in English), number words, mixes ("3 mil", "two and a half million"), negatives | `Value::Number(Number)`, exact |
| `date` | "hoy", "mañana", "pasado mañana", "ayer", weekdays with "este"/"próximo"/"pasado"/"que viene", "dentro de 3 días", "hace dos semanas", "el 3 de mayo", "3/5" (day first in Spanish), "el día 3", "a finales de mes" and their English equivalents, relative to the turn's `now` | `Value::Date { date, year_given }` |
| `duration`, `datetime`, `daterange`, `place`, `language`, `choice`, `confirm` | see [SKILLS.md](SKILLS.md) §3 and conversation.md §2; a `choice` slot lists its `options` | `Value::Duration`, `DateTime`, `DateRange`, `Place`, `Language`, `Choice`, `Confirm` |

A capture that cannot be read becomes `Value::Invalid { raw, problem }`:
`Problem::Unreadable` (malformed, `1,500,000` in Spanish), `Ambiguous(readings)`
(English `3/4`), `DoesNotExist` ("31 de abril"). A slot the message did not
give is simply absent from `call.slots`. The extractors are in
`talos_core::extract` and you may call them on a `term` yourself.

A place has no gazetteer yet: the `place` extractor keeps a name as typed
and resolves only references ("aquí", "home") and time zones; checking that
a name is a real place is the skill's job. "home"/"mi ciudad" is the place
in `cx.preferences().place()`.

**Guesses.** A slot may declare `guess = { before = […], after = […], words
= { value = […] } }`: where to find it, next to cue words, in a message the
classifier recognised (SKILLS.md §1). Without it, a guess carries no value
for the slot.

**Internal slots** (`internal = true`) tell the handler which phrasing was
used (smalltalk's `register`, `daypart`). They are in `call.slots` but not in
the outcome, and golden files leave them out unless `Given: slots = all`.

### 2.3 Replies

`[reply.<id>]` holds one or more **variants** per language. Templates use
`{name}` for variables and `{{`/`}}` for literal braces. Variant 1 is the
canonical wording: a conversation's first turn always gets it; later turns
step through the variants, so two successive turns never read the same.
Variants differ in wording only, never in content, and every variant of a
reply should use the same variables.

### 2.4 Courtesies

`courtesy = true` marks an intent whose phrasing may open or close a message
carrying a real request (greetings, thanks). When the whole message matches
no rule, a courtesy of up to four words at its start or end is dropped if
the rest matches: "hola, qué hora es" reaches your clock rule as "qué hora
es". Your patterns need not allow greetings around them.

### 2.5 Commands

`command = { es = "/diasemana", en = "/weekday", slot = "date" }` gives the
intent a shortcut ([ADR 0016](adr/0016-command-shortcuts.md)): `/weekday
25/12` reaches your handler exactly as "what day is 25/12" would, with the
rest of the line read by the `date` slot's extractor. Names are `/` and
lower-case ASCII, one per language (they may be equal, as `/calc` is); both
work in either conversation language. Leave `slot` out when the intent takes
no argument (`/refran`). Give a command only where it helps: an intent with
several independent slots gets none unless one free-text slot its handler
already reads covers them; that slot may be `internal`, and the handler
reports the sentence form's slots from it (`/dado 2d6` → `count=2
sides=6`). When the skill's other intents take sentences the command should
also reach, add `sentence = true`: the rest of the line is first read by the
rules of the command's own skill, and if one of another of its intents
matches, that intent answers with what its rule captured, as the sentence
would (`/calc 15% de 80` is `calc.percent`, `/calc 5 km a millas` is
`calc.convert`); otherwise the slot takes the line. `talos skills check`
rejects a name declared twice, a slot the intent does not declare, and
`sentence` without a `slot`.

## 3. The handler

```rust
pub trait Skill: Send + Sync {
    fn id(&self) -> &'static str;
    fn replies(&self) -> &'static [&'static str];
    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult;
}
```

- `id` is the manifest's id.
- `replies` lists **every** reply id `handle` can return. `talos skills
  check` fails if one is missing from the manifest or if a manifest reply is
  not listed (unreachable); the engine turns a message with an unlisted id
  into an honest failure.
- `handle` is called with an intent of yours that a rule, a command, a
  taught phrase or the classifier recognised (`cx.guessed()` tells the
  last apart). A guess has no captures: declare `guess` cues on the slots it
  should find (§2.2), and decline when what you need is missing rather than
  guess further. An intent that changes something (`classifier::
  SIDE_EFFECTS`) is offered on a guess only so that you can ask a yes/no
  that restates the change.

### 3.1 What a handler receives

`Call` is the request:

| Field | |
|---|---|
| `intent: &IntentId` | one of your intents; `match call.intent.as_str()` |
| `lang: Lang` | the language of the turn, already decided: reply in it |
| `slots: &Slots` | typed values by slot name (`BTreeMap<String, Value>`), internal ones included |
| `text: &str` | the message exactly as received, for a term the slots do not cover; never for deciding what was asked |

`Context` is everything else, and nothing more:

| Method | |
|---|---|
| `now() -> Timestamp` | the injected instant; `local_date()`, `local_hour()`, `local_seconds_of_day()` give the wall clock when its offset is known (`None` otherwise: answer time-neutrally) |
| `place() -> Option<&Place>` | the caller's place, uninterpreted until Slice C |
| `random() -> &mut Random` | the turn's random source; see §3.4 |
| `conversation() -> &ConversationState` | `turn()` (1-based), `lang()`, `last_intent()` (the recent topic) |
| `preferences() -> &Preferences` | what the user asked to be remembered: `name()`, `place()` (as typed, a leading capital added), `units()` (`Units::Metric`/`Imperial`), each an `Option` |
| `guessed() -> bool` | the classifier, not a rule, recognised the request; it has only the slots its manifest's `guess` cues found |
| `remember(PreferenceChange)` | asks the engine to store (`Some`) or forget (`None`) a preference; applied only if you answer (`Answer`), and dropped, with the turn failed, if made on a guess without a `confirm` slot answered yes |
| `skills() -> &[Manifest]` | the installed manifests in catalogue order (for `help`) |
| `report(name, Value)` | puts a slot on the outcome: a default you applied or a value you resolved. It must be a declared, non-internal slot of the intent, or the turn fails |

### 3.2 What a handler returns

| `SkillResult` | Status | Use for |
|---|---|---|
| `Answer(Message)` | `answered` | a complete, correct answer, including a correct "no" or "that is impossible" (dividing by zero, 31 April) |
| `Unable(Message)` | `unhandled`, intent kept | recognised but beyond what Talos supports (an unknown unit, too many dice) |
| `Decline` | — | not yours after all; the next candidate rule is tried |
| `Fail(SkillError)` | `unhandled` | a bug or an impossible internal state; the user gets an honest error |
| `Ask(Expectation, Message)` | `answered`, `pending`; `asks · intent · slot` in transcripts | a required slot is missing, or a value is ambiguous: one short question naming what is missing (`Expectation::slot("target")`, `::choice`, `::confirm`, `::completing`). The answer comes back as a new call with that slot filled; never ask for an optional slot |

The test between `Answer` and `Unable` is the spec's: would a more capable
engine plausibly give the user something better? If yes, `Unable`.

`Message::new("reply_id").var("name", value)` names a reply of your manifest
and fills its variables. Variables are text already written for the turn's
language: format numbers with `Number::format(call.lang)` (Spanish `12.345`
and `1,5`; English `12,345` and `1.5`), dates and times by the conventions in
basics.md. Pre-rendered prose in a variable is the escape hatch for lists
(help's skill list), not a habit.

### 3.3 What a handler may not do

- Build sentences in code, or choose wording by language in code: wording
  lives in the manifest.
- Read the system clock, the environment, files, the network, or any global
  or static mutable state. Everything comes through `Call` and `Context`.
- Keep state between turns. (Conversation state belongs to the engine; ask
  the integrator if you need some. Preferences are changed only through
  `cx.remember`.)
- Draw randomness other than through `cx.random()`.
- Panic, `unwrap`, or loop unboundedly on user input; cap sizes (dice, list
  lengths) and answer `Unable` beyond them.
- Inspect `call.text` to decide which intent was meant (ADR 0007).

### 3.4 Randomness

`cx.random()` returns the turn's `Random`. Every method is **one draw**:

| Method | Returns | Golden `Given: draw =` |
|---|---|---|
| `int(low, high)` | a whole number in `low..=high` | `4`, `-2` |
| `scaled(low, high, places)` | a number with `places` decimals, in units of `10^-places` | `1.73` |
| `coin()` | `Coin::Heads` or `Coin::Tails` | `heads`, `tails` |
| `pick(n)` | a 0-based index below `n` | `option 2` (1-based) |
| `entry(&[&str])` | an index into a bundled list | `entry "fragment"` (matched ignoring case and accents) |

The draws of a turn depend only on the configured seed, the conversation,
the turn number and the text. Consume exactly the documented number of
draws (one per die, coin, pick, number, fortune; basics.md), in order, and
none when the turn needs none.

## 4. Golden transcripts

Transcribe your section of basics.md into `golden/<id>/*.txt`: every example
and edge case a rule can reach, both languages. The format (in
[DEVELOPMENT.md](DEVELOPMENT.md#golden-transcripts)) mirrors the spec's,
with these adjustments:

- the outcome line is canonical: `→ answered · clock.date · day=2027-03-15`,
  slots in name order, values canonical (numbers `1500`/`1.5`, dates ISO,
  lists `a|b|c`), internal slots omitted; `→ unhandled · catch-all` when no
  intent was recognised;
- `Given:` lines are only `now`, `seed`, `default language`, `hint`, `place`,
  `draw`, `slots`; express anything else ("a conversation that has been in
  Spanish") with turns;
- replies are our wording; what must match the spec is content, language and
  outcome;
- a turn that asks back is `→ asks · intent · slot`; the answering turn
  follows it with no blank line (one conversation) and its outcome is the
  original intent, completed;
- spec examples that need another skill are left out, with a `#` comment
  saying why.

Write expected output by hand from the spec, run `cargo test -p
talos-skills --test golden`, and fix code or file until they agree. Use
`TALOS_BLESS=1` only to accept output you have checked.

## 5. Checking your work

```sh
cargo run -q -p talos -- skills check          # zero errors and zero warnings
cargo run -q -p talos -- explain "qué día es mañana"
cargo run -q -p talos -- ask --json "what day is it tomorrow"
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

`talos explain` shows each stage: the match form, how the language was
decided, which rules matched with their priority, the typed slots, your
handler's result, the reply and variant chosen, the draws, and the
conversation state. A test in `talos-skills` fails on any `skills check`
finding.

Unit tests of a handler build a context with
`Context::standalone(now, Random::seeded(0))` (or `Random::scripted(…)`)
and a `Call` by hand; see `smalltalk.rs`.

## 6. Worked example: `smalltalk.greet`

**Manifest** (`skills/smalltalk/manifest.toml`, abridged):

```toml
[skill]
id = "smalltalk"
summary.es = "saludar y despedirme"
summary.en = "say hello and goodbye"
names.es = ["saludos", "charla"]
names.en = ["greetings", "small talk"]
help.es = ["hola", "buenos días"]
help.en = ["hello", "good morning"]

[[intent]]
id = "smalltalk.greet"
priority = 100
courtesy = true

  [intent.examples]
  es = ["hola", "buenas", "buenos días", "buenas tardes", "ey", "muy buenas", "hola talos"]
  en = ["hi", "hello", "hey", "good morning", "good evening", "hiya", "hello talos"]

  [[intent.slot]]
  name = "register"
  type = "term"
  internal = true

  [[intent.slot]]
  name = "daypart"
  type = "term"
  internal = true

  [[intent.pattern]]
  lang = "es"
  regex = '^[¡¿!\s]*(?:hola+|holi+s?|h?ey+|wenas)(?:[\s,]+talos)?[\s!?.]*$'

  [[intent.pattern]]
  lang = "es"
  regex = '^[¡¿!\s]*(?:muy\s+)?buen(?:os)?\s+dias(?:[\s,]+talos)?[\s!?.]*$'
  slots = { daypart = "morning" }

[reply.hello]
es = ["¡Hola!", "¡Hola, buenas!"]
en = ["Hello!", "Hi!"]

[reply.morning]
es = ["¡Buenos días!"]
en = ["Good morning!"]
```

`dias` without an accent matches "días" and "dias". The pattern fixes the
internal slot `daypart`, so the handler knows which greeting it was without
reading the text. `courtesy = true` lets "hola, qué hora es" reach the clock.

**Handler** (`crates/talos-skills/src/smalltalk.rs`, abridged):

```rust
pub(crate) struct Smalltalk;

impl Skill for Smalltalk {
    fn id(&self) -> &'static str {
        "smalltalk"
    }

    fn replies(&self) -> &'static [&'static str] {
        &["hello", "casual", "morning", "afternoon", "evening"]
    }

    fn handle(&self, cx: &mut Context<'_>, call: &Call<'_>) -> SkillResult {
        match call.intent.as_str() {
            "smalltalk.greet" => Self::greet(cx, call),
            _ => SkillResult::Decline,
        }
    }
}

impl Smalltalk {
    fn greet(cx: &Context<'_>, call: &Call<'_>) -> SkillResult {
        if let Some(said) = call.slots.get("daypart").and_then(Value::as_text) {
            // Echo "buenos días" only when the local clock agrees with it.
            let agrees = cx.now().local_hour().is_some_and(|h| agrees(said, call.lang, h));
            return SkillResult::Answer(Message::new(if agrees { reply_for(said) } else { "hello" }));
        }
        SkillResult::Answer(Message::new("hello"))
    }
}
```

**Registration**: `smalltalk::Smalltalk,` in the `skills!` list.

**Golden file** (`golden/smalltalk/greet.txt`, excerpt):

```text
# Given: now defaults to 2027-03-14 18:42 Europe/Madrid.

[es] U: Buenas tardes
     T: ¡Buenas tardes!                           → answered · smalltalk.greet

[es] U: buenos días
     T: ¡Hola!                                    → answered · smalltalk.greet

Given: slots = all

[es] U: buenas
     T: ¡Buenas!                                  → answered · smalltalk.greet · register=casual
```

**Explain**:

```text
$ talos explain "buenos días"
normalise  "buenos días", match form "buenos dias"
language   es: decided by marker words es=2 en=0
rules      matched smalltalk.greet (priority 100, /^[¡¿!\s]*(?:muy\s+)?buen(?:os)?\s+dias…/)
slots      daypart=morning
dispatch   smalltalk.greet → answer `hello`
render     reply `hello`, variant 1 of 2 (turn 1)
random     0 draws
state      conversation "explain": turn 1, language es (settled), recent topic smalltalk.greet
internal   daypart=morning
outcome    [es] answered · smalltalk.greet
reply      ¡Hola!
```

## 7. Checklist

- [ ] `skills/<id>/manifest.toml`: both languages everywhere; `names`,
      `help`, `summary`; patterns lower case, unaccented, anchored; every
      capture a declared slot.
- [ ] `crates/talos-skills/src/<id>.rs`: `replies()` complete; no sentence
      built in code; no clock, file, environment or global state; draws only
      through `cx.random()`, as many as documented.
- [ ] Your line uncommented in the `skills!` list, and nothing else shared
      touched.
- [ ] `golden/<id>/`: every reachable example and edge case of your spec
      section, both languages.
- [ ] Models retrained (`cargo run --release -p talos-train`) after any
      change to examples.
- [ ] `talos skills check` clean; `cargo test`, `clippy -D warnings`, `fmt`
      clean.
