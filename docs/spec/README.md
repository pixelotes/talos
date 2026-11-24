# Behavioural specification

What Talos must do, described as observable behaviour. Implementation works
from these documents ([ADR 0002](../adr/0002-clean-room-rewrite.md)): each
skill is built to satisfy its specification, and each example here is a
candidate golden transcript.

The specification says **what** is recognised and **what** is answered. It
does not say how: no regular expressions, no code, no data structures.

## Files

| File | Covers |
|---|---|
| [conversation.md](conversation.md) | cross-cutting behaviour: language, slots, open questions, cancellation |
| [basics.md](basics.md) | `smalltalk`, `identity`, `help`, `clock`, `calc`, `random`, `fortune` |
| [state.md](state.md) | `timezones`, `preferences` |
| [knowledge.md](knowledge.md) | `define`, `relations`, `teach`, `translate` |
| [reflector.md](reflector.md) | the fallback, and what is `Deflected` vs `Unhandled` |
| [decisions.md](decisions.md) | decisions the specifications took that the design had not settled |

## Intent catalogue

These identifiers are fixed. Specifications, skill manifests and evaluation
batteries all use them. Adding or renaming one is a change to this file first.

| Intent | The user wants… |
|---|---|
| `smalltalk.greet` | to say hello |
| `smalltalk.bye` | to say goodbye |
| `smalltalk.thanks` | to thank Talos |
| `smalltalk.how_are_you` | to ask how Talos is |
| `identity.who` | to know who or what Talos is |
| `identity.nature` | to know whether it is an AI / a model / how it works |
| `help.capabilities` | to know what Talos can do |
| `help.skill` | to know how to use one capability |
| `clock.now` | the time, now or at an offset from now |
| `clock.date` | the date, today or on a relative day |
| `clock.weekday` | the day of the week, today or on a date |
| `clock.until` | how long until (or since) a date or time |
| `timezones.time_in` | the current time in another place |
| `timezones.difference` | the time difference between two places |
| `calc.arithmetic` | the result of an arithmetic expression |
| `calc.percent` | a percentage calculation |
| `calc.convert` | a unit conversion |
| `random.dice` | a dice roll |
| `random.coin` | a coin flip |
| `random.pick` | one choice among options given |
| `random.number` | a random number in a range |
| `fortune.tell` | a saying or fortune |
| `define.word` | what a word means |
| `relations.is_a` | whether X is a kind of Y |
| `relations.kinds` | the kinds of X |
| `relations.parts` | the parts of X |
| `relations.part_of` | what X is part of |
| `relations.causes` | what X causes, or why X happens |
| `relations.about` | everything Talos knows about X |
| `relations.provenance` | how sure Talos is, or where it learned something |
| `teach.assert` | to teach a fact |
| `teach.forget` | to retract a fact |
| `teach.list` | to see what has been taught |
| `teach.phrase` | to teach that a phrase means an intent ("when I say X I mean Y") |
| `translate.word` | a word or stock phrase in the other language |
| `preferences.set` | to have something remembered (name, place, units) |
| `preferences.get` | to ask what Talos remembers about them |
| `preferences.forget` | to have a preference forgotten |

Skills planned for later milestones (`weather`, `search`, `games`) are not
specified yet.

## Format of a skill section

```markdown
## <skill id>

One paragraph: what it is for and where its limits are.

### <intent id>

**Recognises** — the kinds of phrasing that mean this, in prose.
**Slots** — name, type, required or optional, default.
**Answers** — what the reply contains, in prose.
**Does not** — nearby requests that are *not* this intent, and where they go.

#### Examples
(exchanges, see below)

#### Edge cases
(exchanges with a one-line reason each)
```

## Format of an exchange

```text
[es] U: ¿qué hora es en Tokio?
     T: En Tokio son las 03:42.            → answered · timezones.time_in · place=Tokio

[en] U: what's the time in Tokyo?
     T: In Tokyo it's 03:42.               → answered · timezones.time_in · place=Tokyo
```

- `[es]` / `[en]` is the conversation language. Every intent has examples in
  both, written natively — never one translated from the other.
- The arrow line gives the outcome: status (`answered`, `deflected`,
  `unhandled`), intent, and slots.
- Multi-turn exchanges list the turns in order. A turn where Talos asks back
  is marked `→ asks · <slot>`.
- When the reply depends on the clock or on stored knowledge, state the
  assumption above the exchange: `Given: now = 2027-03-14 18:42 Europe/Madrid`.
- Reply wording is illustrative. What is normative is the content of the
  reply, its language, and the outcome line.

## Which status an outcome gets

One rule, applied by every skill:

- **`answered`** — Talos gave a complete and correct response to what was
  asked. That includes a correct "no" and a correct "that cannot be done":
  dividing by zero has no result, 31 April does not exist, a guitar is not a
  wind instrument because a known fact contradicts it.
- **`unhandled`** — Talos could not provide what was asked, and something more
  capable plausibly could: an unknown word or place, an unsupported unit,
  nothing known either way, a request to generate text. When an intent was
  recognised, the outcome line keeps it: `→ unhandled · define.word`.
- **`deflected`** — only the reflector, on a recognised sentence shape.

The test for `answered` versus `unhandled` is: *would passing this turn to a
more capable engine be likely to give the user something better?* If yes, it
is `unhandled`.

A turn where Talos asks back is not a final outcome; it is marked
`→ asks · <slot>`.

## Rules that apply everywhere

- Talos answers in the language of the conversation.
- Talos never pretends. If it cannot do something it says so plainly and the
  status is `unhandled`.
- A request to generate text (a poem, a summary, a plan) is always
  `unhandled`.
- Replies are short: one or two sentences unless a list was asked for.
- No reply asks how the user feels, offers unsolicited suggestions, or comments
  on the user.
