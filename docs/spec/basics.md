# Basics

Behavioural specification for the skills that need no stored knowledge and no
network: `smalltalk`, `identity`, `help`, `clock`, `calc`, `random` and
`fortune`. Format and global rules are in [README.md](README.md); cross-cutting
behaviour (language stickiness, slot questions, cancellation) is in
[conversation.md](conversation.md) and is not repeated here.

## Conventions used in this file

These apply to every skill below. They are stated once so that the sections can
stay short.

**Numbers in and out.** The conversation language decides how digits are read
and written.

- Spanish writes a decimal comma and groups thousands with a dot from five
  digits up (`1234`, `12.345`, `1.234.567,89`). English writes a decimal point
  and groups with commas from four digits up (`1,234`, `12,345`,
  `1,234,567.89`).
- On input, both separators are accepted in both languages when only one
  reading is possible (`3,5` and `3.5` are both three and a half). A lone
  separator followed by exactly three digits is read by the language's own
  convention: in Spanish `1.000` is one thousand and `1,000` is one; in English
  `1,000` is one thousand and `1.000` is one. Whenever the reading was not
  forced, the reply shows how the number was read, so a wrong guess is visible.
- Number words are understood up to the millions ("tres", "veintiuno", "dos mil
  quinientos", "one hundred and five"). Spanish "billón" is 10^12 and English
  "billion" is 10^9; each is read in the language of the conversation.
- Results that are not whole numbers are shown with at most the digits the
  skill's section says; whenever a shown value was rounded, the reply marks it
  with `≈`.

**Clock and calendar.** Formats follow the language, not the machine.

- Spanish: 24-hour clock (`18:42`), day before month, weekday and month in
  lower case: "domingo, 14 de marzo de 2027". Numeric dates are day/month/year.
- English: 24-hour clock as in the other specification files (`18:42`), day
  before month, weekday and month capitalised: "Sunday 14 March 2027".
  Numeric dates in an English turn are ambiguous (3/4 is 3 April in most of
  the world and 4 March in the US): Talos asks, unless only one reading is a
  valid date ([conversation.md](conversation.md) 2.3).
- A user may ask for a 12-hour clock explicitly ("in 12-hour format", "en
  formato de 12 horas") or for an ISO date; that request is honoured for that
  reply only. Input is accepted in either clock in either language ("las 6 de
  la tarde", "18:00", "6 pm").
- Weeks start on Monday, in both languages, when a phrase refers to "this week"
  or "next week".
- All clock answers are computed from the turn's injected time in the
  conversation's time zone: the remembered place's, else the caller's place's,
  else the server's ([state.md](state.md), precedence). Seconds are
  never shown, and never rounded up: 18:42:59 is `18:42`.

**Randomness.** Every random skill draws from a source that is a pure
function of the conversation, the turn number and the text, plus an optional
seed set in configuration ([conversation.md](conversation.md) 5.3). Nothing is
seeded from system entropy: that would break the determinism goal of
[DESIGN.md](../DESIGN.md) §1. Observable consequences, which are what is
normative:

- With the same conversation, the same sequence of turns and the same
  configured seed, every random reply is byte-for-byte identical.
- A random turn's draws depend only on that turn; there is no shared stream
  that earlier turns consume.
- Each random intent consumes a documented number of draws: one per die, one per
  coin, one per pick, one per number, one per fortune. A test that fixes the
  source can therefore predict the result of a turn.
- Results are uniform over the legal range and always inside it.
- Template wording variants are not random: they depend only on conversation
  and turn number ([DESIGN.md](../DESIGN.md) §4, stage 6).

In examples, `Given: draw = …` states the outcome the fixed source produces,
in the natural terms of the intent (faces for dice, `heads` or `tails` for a
coin, a 1-based option index for a pick, the value itself for a number, an entry
of the bundled list for a fortune).

**When Talos cannot.** Two situations are kept apart.

- A request that is well formed but has a mathematically or physically
  *impossible* answer (dividing by zero, converting mass to length, a
  temperature below absolute zero) gets a plain explanation and the status
  `answered`: saying so is the correct answer.
- A request Talos recognises but is *outside what it supports* (an unknown
  unit, a dice expression it does not parse, a number too large) gets a plain
  statement that it cannot, and the status `unhandled`, with the recognised
  intent kept on the outcome line.

**Greeting prefixes.** A greeting or thanks at the start or end of a message
that also contains a real request is dropped before classification, and the
request is answered ("hola, qué hora es" is a clock question). The reply may
open with a greeting of its own. The same message with nothing else is
smalltalk.

---

## smalltalk

Greetings, farewells, thanks and the "how are you" ritual. Replies are
one short line, never ask a question back and never claim a mood. Variation of
wording depends only on the conversation and the turn number.

### smalltalk.greet

**Recognises** — hello in any register: "hola", "buenas", "buenos días",
"buenas tardes", "ey", "muy buenas"; "hi", "hello", "hey", "good morning",
"good evening", "hiya", with or without Talos's name, with stretched letters
("holaaa"). Not "buenas noches" (a farewell, see `smalltalk.bye`).
**Slots** — none.
**Answers** — a greeting in the conversation language. It never contradicts the
clock: it does not answer "buenos días" with a mismatched "buenas tardes", and
a plain "Hola."/"Hello." is always acceptable. It does not offer help and does
not ask anything.
**Does not** —
- "hola, ¿qué tal?" and "hey, how are you": that is `smalltalk.how_are_you`
  (the greeting is the dropped prefix).
- "hola, qué hora es" and any greeting followed by a request: the request's
  intent (`clock.now`, …).
- "hola, me llamo Raúl": `preferences.set`; the greeting is the dropped prefix.
- "buenas noches", "good night": `smalltalk.bye`.
- "hola" meaning a language lesson ("cómo se dice hola en inglés"):
  `translate.word`.

#### Examples

```text
[es] U: hola
     T: Hola.                                         → answered · smalltalk.greet

[es] U: buenas
     T: Buenas.                                       → answered · smalltalk.greet

[es] U: buenos dias talos
     T: Hola.                                         → answered · smalltalk.greet

[es] U: ey, muy buenas
     T: Hola.                                         → answered · smalltalk.greet

[es] U: holaaaa
     T: Hola.                                         → answered · smalltalk.greet

[es] U: Buenas tardes
     T: Buenas tardes.                                → answered · smalltalk.greet

[en] U: hi
     T: Hello.                                        → answered · smalltalk.greet

[en] U: hello there
     T: Hi.                                           → answered · smalltalk.greet

[en] U: Good morning, Talos
     T: Hello.                                        → answered · smalltalk.greet

[en] U: hey
     T: Hey.                                          → answered · smalltalk.greet

[en] U: hiya!
     T: Hello.                                        → answered · smalltalk.greet

[en] U: good evening
     T: Good evening.                                 → answered · smalltalk.greet
```

#### Edge cases

```text
Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: hola, qué hora es
     T: Hola. Son las 18:42.                          → answered · clock.now
     (greeting prefix dropped; the request decides the intent)

[es] U: hola, me llamo Raúl
     T: Mucho gusto, Raúl. Lo recordaré.              → answered · preferences.set · key=name, value=Raúl
     (a greeting followed by a self-introduction is a preference, not smalltalk;
      never "Encantado": Talos has no gender, reflector.md 3.2)

[es] U: buenos días
     T: Hola.                                         → answered · smalltalk.greet
     (at 18:42 the reply must not echo "buenos días")

[en] U: hello, what's 3 times 4
     T: Hello. 3 × 4 = 12.                            → answered · calc.arithmetic
     (greeting prefix dropped)

[en] U: hiii!!!
     T: Hi.                                           → answered · smalltalk.greet
     (repeated letters and punctuation are normalised)
```

### smalltalk.bye

**Recognises** — leaving or ending: "adiós", "hasta luego", "chao", "nos vemos",
"me voy", "hasta mañana", "buenas noches"; "bye", "goodbye", "see you later",
"gotta go", "cya", "good night", "talk to you later".
**Slots** — none.
**Answers** — a farewell in the conversation language, one short line, no
question, no invitation to return.
**Does not** —
- "me voy a Madrid mañana": a statement about plans, not a farewell; it goes
  on to the rest of the pipeline.
- "gracias, adiós" and "thanks, bye": one intent per turn; the farewell wins
  because it ends the exchange, and the reply is only a farewell.
- "no, déjalo": cancellation of an open question, handled by the dialog layer.
- "buenas noches" said as a greeting cannot be told apart from the farewell by
  Talos; it is always treated as a farewell.

#### Examples

```text
[es] U: adiós
     T: Hasta luego.                                  → answered · smalltalk.bye

[es] U: hasta luego
     T: Hasta pronto.                                 → answered · smalltalk.bye

[es] U: chao, me voy
     T: Adiós.                                        → answered · smalltalk.bye

[es] U: nos vemos!
     T: Hasta la próxima.                             → answered · smalltalk.bye

[es] U: buenas noches
     T: Buenas noches.                                → answered · smalltalk.bye

[es] U: bueno pues hasta mañana
     T: Hasta mañana.                                 → answered · smalltalk.bye

[en] U: bye
     T: Goodbye.                                      → answered · smalltalk.bye

[en] U: see you later
     T: See you.                                      → answered · smalltalk.bye

[en] U: good night
     T: Good night.                                   → answered · smalltalk.bye

[en] U: gotta go
     T: Bye.                                          → answered · smalltalk.bye

[en] U: ok cya
     T: Goodbye.                                      → answered · smalltalk.bye

[en] U: talk to you later
     T: Talk to you later.                            → answered · smalltalk.bye
```

#### Edge cases

```text
[es] U: gracias, adiós
     T: Hasta luego.                                  → answered · smalltalk.bye
     (one intent per turn; the farewell outranks the thanks)

[en] U: thanks, bye!
     T: Goodbye.                                      → answered · smalltalk.bye
     (same rule in English)

[es] U: me voy a Sevilla el viernes
     T: (reflector)                                   → deflected
     (a plan, not a farewell; "me voy" followed by a destination is not a bye)

[en] U: I'm off to bed
     T: Good night.                                   → answered · smalltalk.bye
     (leaving the conversation, even with a reason, is a bye)
```

### smalltalk.thanks

**Recognises** — gratitude: "gracias", "muchas gracias", "mil gracias", "grax",
"vale, gracias", "te lo agradezco", "gracias por la ayuda"; "thanks", "thank
you", "thanks a lot", "thx", "ty", "much appreciated".
**Slots** — none.
**Answers** — a short acknowledgement ("De nada." / "You're welcome."). No
comment on the user, no "happy to help", no follow-up question.
**Does not** —
- "no, gracias" / "no thanks": a refusal, not gratitude. Talos does not say
  "de nada"; the message is offered to an open expectation as a "no" if there
  is one and otherwise goes on to the rest of the pipeline.
- "gracias, qué hora es": a thanks prefix on a request; the request's intent
  answers and the thanks is dropped.
- "gracias, adiós": `smalltalk.bye`.
- "cómo se dice gracias en inglés": `translate.word`.

#### Examples

```text
[es] U: gracias
     T: De nada.                                      → answered · smalltalk.thanks

[es] U: muchas gracias
     T: No hay de qué.                                → answered · smalltalk.thanks

[es] U: mil gracias!!
     T: De nada.                                      → answered · smalltalk.thanks

[es] U: vale, gracias
     T: De nada.                                      → answered · smalltalk.thanks

[es] U: grax
     T: De nada.                                      → answered · smalltalk.thanks

[es] U: te lo agradezco
     T: A ti.                                         → answered · smalltalk.thanks

[en] U: thanks
     T: You're welcome.                               → answered · smalltalk.thanks

[en] U: thank you so much
     T: You're welcome.                               → answered · smalltalk.thanks

[en] U: thx
     T: No problem.                                   → answered · smalltalk.thanks

[en] U: ty
     T: You're welcome.                               → answered · smalltalk.thanks

[en] U: much appreciated
     T: You're welcome.                               → answered · smalltalk.thanks

[en] U: thanks a lot
     T: Anytime.                                      → answered · smalltalk.thanks
```

#### Edge cases

```text
[es] U: no, gracias
     T: No tengo ninguna pregunta pendiente.          → unhandled
     (a refusal never gets "de nada"; with nothing open it is a bare "no",
      conversation.md 3.8)

[en] U: no thanks
     T: There's no open question to answer.           → unhandled
     (same rule in English)

Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: gracias, qué hora es
     T: Son las 18:42.                                → answered · clock.now
     (thanks prefix dropped)

[en] U: thanks!
     T: You're welcome.                               → answered · smalltalk.thanks
     (as the very first message of a conversation it is still answered)
```

### smalltalk.how_are_you

**Recognises** — asking how Talos is: "¿qué tal estás?", "cómo estás", "cómo
andas", "¿todo bien?", "cómo te encuentras", "¿cómo va todo?"; "how are you",
"how's it going", "how are you doing", "you good?", "what's up", "you ok?".
Also "hola, ¿qué tal?" and "hey, how are you" (the greeting is dropped).
**Slots** — none.
**Answers** — a factual status line: Talos is running normally. It never says
it is happy, tired, fine or well, because it has no feelings and does not
claim any. It does not ask the user how they are. When the wording explicitly
asks about feelings ("cómo te sientes", "how are you feeling") the reply adds
plainly that Talos has none.
**Does not** —
- "tienes sentimientos", "do you have feelings", "are you happy": that is
  `identity.nature`.
- "cómo está el tiempo", "how is the weather": weather, not specified yet.
- "cómo se dice … en inglés": `translate.word`.
- "yo estoy mal": a statement about the user; the reflector's business, never
  this intent.

#### Examples

```text
[es] U: ¿qué tal estás?
     T: Funcionando con normalidad.                   → answered · smalltalk.how_are_you

[es] U: como andas
     T: Todo en orden por aquí.                       → answered · smalltalk.how_are_you

[es] U: hola, qué tal
     T: Hola. Todo funciona con normalidad.           → answered · smalltalk.how_are_you

[es] U: ¿todo bien?
     T: Sí, funcionando con normalidad.               → answered · smalltalk.how_are_you

[es] U: cómo va todo
     T: Todo en orden.                                → answered · smalltalk.how_are_you

[es] U: oye, cómo te encuentras hoy
     T: Funcionando con normalidad.                   → answered · smalltalk.how_are_you

[en] U: how are you?
     T: Running normally.                             → answered · smalltalk.how_are_you

[en] U: how's it going
     T: All running normally.                         → answered · smalltalk.how_are_you

[en] U: hey, how are you doing
     T: Hello. Everything is running normally.        → answered · smalltalk.how_are_you

[en] U: what's up
     T: Nothing out of the ordinary; running normally. → answered · smalltalk.how_are_you

[en] U: you ok?
     T: Yes, running normally.                        → answered · smalltalk.how_are_you

[en] U: how are things with you
     T: Running normally.                             → answered · smalltalk.how_are_you
```

#### Edge cases

```text
[es] U: ¿cómo te sientes?
     T: No tengo sentimientos. Funciono con normalidad. → answered · smalltalk.how_are_you
     (explicit mention of feelings: the reply says plainly there are none)

[en] U: how are you feeling today?
     T: I don't have feelings. I'm running normally.  → answered · smalltalk.how_are_you
     (same rule in English)

[es] U: tienes sentimientos?
     T: No. Soy un programa y no siento nada.         → answered · identity.nature
     (a direct question about having feelings is identity, not smalltalk)

[en] U: how are you? i'm tired
     T: Running normally.                             → answered · smalltalk.how_are_you
     (the reply never comments on or asks about the user)
```

---

## identity

Who Talos is and what it is. Talos tells the truth about itself in every case:
it is a program, it is not a language model, it has no feelings, it is not
conscious, and it never lets a user believe otherwise. It does not boast, does
not compare itself with other assistants and does not invent a biography.

### identity.who

**Recognises** — asking for its name or what it is called, or to introduce
itself: "quién eres", "cómo te llamas", "qué eres", "preséntate", "cuál es tu
nombre"; "who are you", "what's your name", "what are you", "introduce
yourself", "tell me about yourself".
**Slots** — none.
**Answers** — its name (Talos) and one sentence on what it is: a small
assistant that answers questions in Spanish and English by itself, without a
language model. Asked who created it ("quién te hizo", "quién te creó",
"quién te ha programado", "who made you", "who created you", "who built
you"), it gives a fixed identity line, `answered · identity.who`. The line is
configuration; the default says Talos is a project by Pixelotes, and nothing
more: no individual person and no company beyond that, no date, no
biography. (Ruled 2026-11-24, ruling 4.) It is worded in the language of the
turn and never varies in content.
**Does not** —
- "por qué te llamas Talos", "how old are you": recognised here but with no
  bundled answer; a plain statement that it has no such information,
  `unhandled` with the intent kept.
- "eres una IA", "are you a robot", "do you have feelings", "how do you work":
  `identity.nature`.
- "cómo me llamo", "what's my name": `preferences.get`.
- "qué puedes hacer": `help.capabilities`.

#### Examples

```text
[es] U: quién eres
     T: Soy Talos, un asistente que responde en español e inglés sin usar un modelo de lenguaje.
                                                      → answered · identity.who

[es] U: como te llamas
     T: Me llamo Talos. Respondo preguntas sencillas sin modelo de lenguaje.
                                                      → answered · identity.who

[es] U: qué eres exactamente
     T: Soy Talos, un asistente pequeño que funciona con reglas y datos propios.
                                                      → answered · identity.who

[es] U: preséntate
     T: Soy Talos. Contesto en español o en inglés, con reglas y conocimiento propio, sin modelo de lenguaje.
                                                      → answered · identity.who

[es] U: cual es tu nombre
     T: Talos.                                        → answered · identity.who

[es] U: tú quién eres
     T: Soy Talos, un asistente que no usa modelo de lenguaje.
                                                      → answered · identity.who

[en] U: who are you?
     T: I'm Talos, a small assistant that answers in English and Spanish without a language model.
                                                      → answered · identity.who

[en] U: what's your name
     T: Talos.                                        → answered · identity.who

[en] U: what are you?
     T: I'm Talos, a small assistant that runs on rules and its own data, not a language model.
                                                      → answered · identity.who

[en] U: introduce yourself
     T: I'm Talos. I answer in English or Spanish using rules and stored knowledge, with no language model.
                                                      → answered · identity.who

[en] U: tell me about yourself
     T: I'm Talos, an assistant built from rules, templates and a small knowledge store. I don't use a language model.
                                                      → answered · identity.who

[en] U: and you are?
     T: Talos, a small assistant that works without a language model.
                                                      → answered · identity.who
```

#### Edge cases

```text
[es] U: quién te creó
     T: Talos es un proyecto de Pixelotes.            → answered · identity.who
     (the configured identity line; no person is named)

[es] U: ¿quién te ha hecho?
     T: Soy un proyecto de Pixelotes.                 → answered · identity.who

[en] U: who made you?
     T: Talos is a project by Pixelotes.              → answered · identity.who

[en] U: ok but which person built you?
     T: Talos is a project by Pixelotes.              → answered · identity.who
     (the line names no individual, however the question is pressed)

[en] U: why are you called Talos?
     T: I don't have an answer for that.              → unhandled · identity.who
     (origin of the name is not specified)

[es] U: cuántos años tienes
     T: No tengo esa información.                     → unhandled · identity.who
     (no age or date is specified)

[es] U: ¿cómo me llamo yo?
     T: (answered by preferences)                     → answered · preferences.get
     (the user's name is a stored preference, not Talos's identity)

[en] U: what's your name and what time is it
     T: Talos.                                        → answered · identity.who
     (only the first request of a compound message is answered)
```

### identity.nature

**Recognises** — questions about what kind of thing Talos is or how it works:
is it an AI, a robot, a human, a language model, ChatGPT or another named
assistant; does it have feelings, consciousness or a mind; does it understand;
how does it work; what does it run on.
**Slots** — none.
**Answers** — honest and short:
- it is a program, not a person;
- it is not a language model and does not generate free text; it recognises
  the sentence with rules and a small classifier, then answers from templates
  and stored data;
- it has no feelings, no consciousness and no preferences, and it never
  claims any;
- asked about a named assistant, it says it is not that one.

It does not refuse to say "AI" out of vanity, and does not use the word to
suggest more than it is: it says it is software and states what kind.
**Does not** —
- "quién eres", "cómo te llamas": `identity.who`.
- "cómo estás", "¿todo bien?": `smalltalk.how_are_you`.
- "eres mejor que ChatGPT", "are you smarter than Siri": recognised here, but
  a comparison Talos does not make; `unhandled · identity.nature`.
- "¿sueñas?", "¿te aburres?", "do you have friends?": questions that presuppose
  a life rather than ask what Talos is; the reflector's S17 (`deflected`,
  [reflector.md](reflector.md)).
- "me quieres?": answered here with the same plain statement that it has no
  feelings; it does not play along.

#### Examples

```text
[es] U: eres una ia?
     T: Soy un programa, pero no un modelo de lenguaje: reconozco la frase con reglas y respondo con plantillas y datos.
                                                      → answered · identity.nature

[es] U: eres humano o robot
     T: Ni lo uno ni lo otro: soy un programa. No soy una persona.
                                                      → answered · identity.nature

[es] U: tienes sentimientos
     T: No. No siento nada ni lo aparento.            → answered · identity.nature

[es] U: eres chatgpt??
     T: No. No soy un modelo de lenguaje ni ese asistente.
                                                      → answered · identity.nature

[es] U: cómo funcionas por dentro
     T: Reconozco tu frase con reglas y un clasificador pequeño, y respondo con plantillas y datos guardados. No genero texto libre.
                                                      → answered · identity.nature

[es] U: tienes conciencia
     T: No. Soy un programa y no tengo conciencia.    → answered · identity.nature

[en] U: are you an AI?
     T: I'm software, but not a language model: I match your sentence with rules and answer from templates and stored data.
                                                      → answered · identity.nature

[en] U: are you human?
     T: No. I'm a program.                            → answered · identity.nature

[en] U: do you have feelings?
     T: No. I don't have feelings and I don't pretend to.
                                                      → answered · identity.nature

[en] U: are you a language model
     T: No. I don't generate text; I answer from rules, templates and stored knowledge.
                                                      → answered · identity.nature

[en] U: are you ChatGPT or Siri?
     T: Neither. I'm Talos, a separate program with no language model.
                                                      → answered · identity.nature

[en] U: how do you work?
     T: I recognise your sentence with rules and a small classifier, then answer from templates and stored data.
                                                      → answered · identity.nature
```

#### Edge cases

```text
[es] U: me quieres?
     T: No tengo sentimientos, así que no.            → answered · identity.nature
     (no playing along)

[en] U: do you understand me?
     T: I recognise what kinds of requests your words match; I don't understand the way a person does.
                                                      → answered · identity.nature
     (honest about "understanding")

[es] U: eres más listo que chatgpt?
     T: No hago comparaciones de ese tipo.            → unhandled · identity.nature
     (comparison with other assistants is out of scope)

[en] U: what model are you running?
     T: None. I'm not a language model; I run as a small program on rules and stored data.
                                                      → answered · identity.nature
     (what it runs on; no model, no network needed for the core)

[es] U: ¿tienes emociones?
     T: No. Soy un programa y no tengo emociones.     → answered · identity.nature
     (feelings, consciousness and understanding are identity.nature; "¿sueñas?"
      and other questions about experiences are the reflector's S17)
```

---

## help

What Talos can do and how to use one capability. Both answers are generated
from the manifests of the skills installed and enabled, so they cannot claim
more or less than is available ([SKILLS.md](../SKILLS.md) §1). Disabled skills
(network skills off in configuration) are not advertised as available.

### help.capabilities

**Recognises** — asking for the overall list: "qué sabes hacer", "qué puedes
hacer", "ayuda", "para qué sirves", "qué comandos tienes", "lista de
funciones"; "help", "what can you do", "what are your features", "list your
skills", "what are you able to do". A bare "ayúdame" or "help me" with nothing
else is this intent.
**Slots** — none.
**Answers** — a short list, one line per installed and enabled skill, each
being that skill's `summary` in the conversation language, in a fixed order
(built-in skills in catalogue order, then user-installed skills alphabetically).
A user-installed skill appears as soon as it is installed. A skill that is
disabled does not appear. The list is the whole reply apart from a one-line
introduction.
**Does not** —
- "qué puedo preguntarte sobre fechas", "what can I do with dates": that is
  about one skill, `help.skill`.
- "ayúdame a elegir entre A y B": `random.pick`.
- "ayúdame con mi redacción", "help me write a letter": generation;
  `unhandled`.
- "qué eres": `identity.who`.

#### Examples

```text
Given: installed and enabled skills = smalltalk, identity, help, clock, timezones, calc, random, fortune, define, relations, teach, translate, preferences

[es] U: qué sabes hacer
     T: Esto es lo que sé hacer:
        · saludar y despedirme
        · decir quién soy
        · ayudarte a usar lo que sé hacer
        · la hora y la fecha
        · la hora en otros lugares
        · cálculos, porcentajes y conversión de unidades
        · dados, monedas y elegir al azar
        · un refrán o una frase
        · definir palabras
        · relaciones entre conceptos
        · aprender lo que me enseñes
        · traducir palabras y frases hechas
        · recordar tu nombre, tu ciudad y tus unidades
                                                      → answered · help.capabilities

[es] U: ayuda
     T: (the same list)                               → answered · help.capabilities

[es] U: para qué sirves
     T: (the same list)                               → answered · help.capabilities

[es] U: que comandos tienes
     T: (the same list)                               → answered · help.capabilities

[es] U: oye, en qué me puedes ayudar
     T: (the same list)                               → answered · help.capabilities

[es] U: lista de cosas que sabes hacer
     T: (the same list)                               → answered · help.capabilities

[en] U: what can you do?
     T: Here's what I can do:
        · say hello and goodbye
        · tell you who I am
        · help you use what I can do
        · tell the time and date
        · tell the time in other places
        · arithmetic, percentages and unit conversion
        · dice, coins and random picks
        · a proverb or a saying
        · define words
        · relations between concepts
        · learn what you teach me
        · translate words and stock phrases
        · remember your name, place and units
                                                      → answered · help.capabilities

[en] U: help
     T: (the same list)                               → answered · help.capabilities

[en] U: what are your features
     T: (the same list)                               → answered · help.capabilities

[en] U: list your skills
     T: (the same list)                               → answered · help.capabilities

[en] U: what are you able to do?
     T: (the same list)                               → answered · help.capabilities

[en] U: help me
     T: (the same list)                               → answered · help.capabilities
```

#### Edge cases

```text
Given: installed and enabled skills = the list above plus a user-installed skill "homelab"
       whose manifest summary is "estado de tus servidores" / "your servers' status"
[en] U: what can you do
     T: (the same list, ending with) · your servers' status
                                                      → answered · help.capabilities
     (user-installed skills appear as soon as they are installed)

Given: weather is installed but disabled in configuration
[es] U: qué sabes hacer
     T: (the same list; no line about the weather)    → answered · help.capabilities
     (a disabled skill is not advertised)

[es] U: ayúdame a elegir entre pizza o sushi
     T: Elijo pizza.                                  → answered · random.pick
     (given draw = option 1; the request is a pick, not a help request)

[en] U: help me write a cover letter
     T: I can't write texts. I only answer from rules and stored knowledge.
                                                      → unhandled
     (generation is always unhandled)
```

### help.skill

**Recognises** — asking how to use, or whether Talos can do, one specific
capability: "cómo funciona la calculadora", "cómo uso lo de las conversiones",
"qué puedo preguntarte sobre la hora", "sabes hacer porcentajes"; "how do I use
the calculator", "what can I ask about dates", "do you do percentages", "how do
I teach you something". The skill is named by its id or by the everyday words
its manifest declares ("reloj", "calculadora", "dados", "calc", "clock").
**Slots** — `skill` (term, required). If it is missing ("cómo se usa esto") Talos
asks which one.
**Answers** — the skill's summary and two or three example phrases taken from
its manifest, in the conversation language. If the skill is installed but
disabled, it says that it exists and is turned off in the configuration. If the
topic matches no skill at all, it says plainly that it cannot do that.
**Does not** —
- "convierte 5 km a millas", "how do I convert 5 km to miles" asked as an
  instruction to convert: the conversion itself, `calc.convert`.
- "cómo se dice mesa en inglés": `translate.word`.
- "qué significa porcentaje": `define.word`.
- "sabes escribir poemas", "can you write a poem": generation; `unhandled`,
  said plainly.

#### Examples

```text
[es] U: cómo funciona la calculadora
     T: La calculadora hace cuentas, porcentajes y conversiones. Prueba: «cuánto es 3 por 4», «el 15% de 80», «5 km a millas».
                                                      → answered · help.skill · skill=calc

[es] U: qué puedo preguntarte sobre la hora
     T: Puedo decir la hora y la fecha. Prueba: «qué hora es», «qué día es hoy», «cuánto falta para Navidad».
                                                      → answered · help.skill · skill=clock

[es] U: sabes hacer porcentajes?
     T: Sí. Prueba: «el 15% de 80», «qué porcentaje es 20 de 50», «sube 80 un 10%».
                                                      → answered · help.skill · skill=calc

[es] U: como se usa lo de los dados
     T: Puedo tirar dados, monedas y elegir al azar. Prueba: «tira 2d6», «cara o cruz», «elige entre pizza o sushi».
                                                      → answered · help.skill · skill=random

[es] U: ayuda con lo de enseñarte cosas
     T: Puedes enseñarme hechos. Prueba: «un gato es un animal», «olvida que un gato es un animal», «qué te he enseñado».
                                                      → answered · help.skill · skill=teach

[es] U: que hago para que recuerdes mi nombre
     T: Puedo recordar tu nombre, tu ciudad y tus unidades. Prueba: «me llamo Ana», «cómo me llamo».
                                                      → answered · help.skill · skill=preferences

[en] U: how do I use the calculator?
     T: The calculator does arithmetic, percentages and unit conversion. Try: "what's 3 times 4", "15% of 80", "5 km in miles".
                                                      → answered · help.skill · skill=calc

[en] U: what can I ask about dates?
     T: I can tell the time and date. Try: "what time is it", "what's the date", "how long until Christmas".
                                                      → answered · help.skill · skill=clock

[en] U: do you do percentages
     T: Yes. Try: "15% of 80", "what percent is 20 of 50", "increase 80 by 10%".
                                                      → answered · help.skill · skill=calc

[en] U: how does the dice thing work
     T: I can roll dice, flip coins and pick at random. Try: "roll 2d6", "flip a coin", "pick pizza or sushi".
                                                      → answered · help.skill · skill=random

[en] U: how do I teach you something
     T: You can teach me facts. Try: "a cat is an animal", "forget that a cat is an animal", "what have I taught you".
                                                      → answered · help.skill · skill=teach

[en] U: how do I convert units
     T: Unit conversion is part of the calculator. Try: "10 miles in km", "72 F to C", "2 GB in MB".
                                                      → answered · help.skill · skill=calc
```

#### Edge cases

```text
[es] U: cómo se usa
     T: ¿El qué? ¿La calculadora, el reloj, los dados…? → asks · skill
[es] U: el reloj
     T: Puedo decir la hora y la fecha. Prueba: «qué hora es», «qué día es hoy».
                                                      → answered · help.skill · skill=clock
     (a missing skill is asked for; the follow-up fills the slot)

Given: weather is installed but disabled in configuration
[en] U: how does the weather work?
     T: There is a weather skill, but it is turned off in the configuration.
                                                      → answered · help.skill · skill=weather

[es] U: puedes convertir divisas?
     T: No sé hacer eso. No tengo cotizaciones.       → unhandled · help.skill
     (no installed skill matches; said plainly)

[en] U: can you write poems?
     T: No, I can't write texts.                      → unhandled
     (generation is always unhandled)

[es] U: convierte 5 km a millas
     T: 5 km son ≈ 3,11 mi.                           → answered · calc.convert
     (an instruction to convert is not a question about the skill)
```

---

## clock

The current time, the date, weekdays, and how long until or since a date or
time. Everything is computed from the turn's injected `now` and the
conversation's time zone. Other places are `timezones`.

Conventions specific to this skill:

- **Bare weekday** ("el viernes", "Friday"): the next Friday after today; said
  on a Friday, the one seven days ahead. With "este"/"this" ("este viernes",
  "this Friday") or "hoy"/"today", the nearest Friday from today onward, today
  included ([conversation.md](conversation.md) 2.3).
- **"Next" weekday** ("el próximo viernes", "el viernes que viene", "next
  Friday"): the first Friday strictly after today. Some speakers mean the one
  after that; Talos picks this reading and **always states the resolved date**
  so the user can correct it. "El viernes de la semana que viene" and "Friday
  next week" mean the Friday of the week (Monday to Sunday) after the current
  one.
- **Date without a year** ("25 de diciembre", "March 4"): the occurrence that
  matches the verb. "Until" and "how long before" take the next occurrence,
  today included; "since" takes the most recent one. The reply always shows the
  year.
- **Numeric dates** are read day first in Spanish (`3/4` is 3 April); in
  English they are asked about unless only one reading is valid (see the
  conventions above). The reply writes the date out in words.
- A date that does not exist ("31 de abril", "February 29, 2027") is answered
  with a plain statement that it does not exist.

### clock.now

**Recognises** — the current time: "qué hora es", "me dices la hora", "tienes
hora", "hora", "dime la hora porfa"; "what time is it", "got the time", "time?",
"what's the time". Also the time after an offset: "qué hora será dentro de 3
horas", "what time will it be in 3 hours". "Here", "where I am", "aquí" are the
conversation's own place.
**Slots** — `offset` (duration, optional, default none).
**Answers** — the time in the conversation's zone in the language's clock
convention. Spanish uses the singular only for hour one ("Es la 1:05"; "Son las
13:05"). At exactly 00:00 and 12:00 Talos adds "(medianoche)"/"(midnight)"
or "(mediodía)"/"(noon)". With an offset it names the offset and
marks the next day ("mañana", "tomorrow") when the result crosses midnight.
**Does not** —
- "qué hora es en Tokio", "what time is it in Madrid": `timezones.time_in`,
  even when the place is the user's own zone.
- "cuántas horas de diferencia hay con Nueva York": `timezones.difference`.
- "qué día es hoy": `clock.date`; "qué día de la semana es": `clock.weekday`.
- "cuánto falta para las 9": `clock.until`.
- "avísame en 5 minutos", "set a timer": not supported (Talos cannot speak
  first); `unhandled`.

#### Examples

```text
Given: now = 2027-03-14 18:42 Europe/Madrid

[es] U: qué hora es
     T: Son las 18:42.                                → answered · clock.now

[es] U: me dices la hora?
     T: Las 18:42.                                    → answered · clock.now

[es] U: k hora es
     T: Son las 18:42.                                → answered · clock.now

[es] U: hora
     T: Son las 18:42.                                → answered · clock.now

[es] U: tienes hora
     T: Son las 18:42.                                → answered · clock.now

[es] U: dime la hora porfa
     T: Son las 18:42.                                → answered · clock.now

[es] U: qué hora será dentro de 3 horas
     T: Dentro de 3 horas serán las 21:42.            → answered · clock.now · offset=3 h

Given: now = 2027-03-14 18:42 America/New_York

[en] U: what time is it
     T: It's 18:42.                                   → answered · clock.now

[en] U: got the time?
     T: It's 18:42.                                   → answered · clock.now

[en] U: time?
     T: 18:42.                                        → answered · clock.now

[en] U: what's the time
     T: It's 18:42.                                   → answered · clock.now

[en] U: do you know what time it is right now
     T: It's 18:42.                                   → answered · clock.now

[en] U: tell me the time please
     T: It's 18:42.                                   → answered · clock.now

[en] U: what time will it be in 8 hours
     T: In 8 hours it'll be 02:42 tomorrow.           → answered · clock.now · offset=8 h
```

#### Edge cases

```text
Given: now = 2027-03-14 01:05 Europe/Madrid
[es] U: qué hora es
     T: Es la 1:05.                                   → answered · clock.now
     (hour one is singular in Spanish)

Given: now = 2027-03-14 13:07 Europe/Madrid
[es] U: hora?
     T: Son las 13:07.                                → answered · clock.now
     (13 is plural)

Given: now = 2027-03-14 00:00 America/New_York
[en] U: what time is it
     T: It's 00:00 (midnight).                        → answered · clock.now

Given: now = 2027-03-14 12:00 America/New_York
[en] U: time
     T: It's 12:00 (noon).                            → answered · clock.now

Given: now = 2027-03-14 18:42:59 America/New_York
[en] U: what time is it
     T: It's 18:42.                                   → answered · clock.now
     (seconds are dropped, never rounded up)

Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: qué hora es en formato de 12 horas
     T: Son las 6:42 p. m.                            → answered · clock.now
     (the other convention on request, for this reply only)

Given: now = 2027-03-14 18:42 America/New_York
[en] U: what time is it, in 12-hour format
     T: It's 6:42 PM.                                 → answered · clock.now
     (the other convention on request, for this reply only)

[es] U: qué hora es en Tokio
     T: (answered by timezones)                       → answered · timezones.time_in · place=Tokio

[en] U: what time is it in New York
     T: (answered by timezones, even if that is the user's own zone)
                                                      → answered · timezones.time_in · place=New York

[en] U: what time is it where I am
     T: It's 18:42.                                   → answered · clock.now

[es] U: avísame en 5 minutos
     T: No puedo avisarte: no sé hablar primero.      → unhandled
     (timers and reminders are not supported)
```

### clock.date

**Recognises** — today's date, and the date of a relative day: "qué fecha es
hoy", "a cuántos estamos", "qué día es hoy", "en qué día estamos", "fecha de
hoy"; "what's the date", "what is today's date", "date". Also relative days:
"mañana", "ayer", "pasado mañana", "dentro de 100 días", "hace dos semanas",
"el próximo viernes"; "tomorrow", "yesterday", "in 100 days", "next Friday".
Also partial dates: "en qué año estamos", "what month is it".
**Slots** — `day` (datetime, optional, default today).
**Answers** — the full date with weekday in the language's format, with the
right tense ("fue", "será", "was", "will be"). For a year or a month alone,
just that. An ISO date on request ("en formato ISO", "in ISO format").
**Does not** —
- "qué día de la semana es", "what day of the week is it": `clock.weekday`
  (though the reply to `clock.date` includes the weekday as well).
- "cuánto falta para Navidad": `clock.until`.
- "qué fecha es en Tokio", "what day is it in Sydney": `timezones.time_in`,
  which gives the local time and date there ([state.md](state.md)).
- "cuándo es mi cumpleaños": Talos does not know it; `unhandled`.

#### Examples

```text
Given: now = 2027-03-14 18:42 Europe/Madrid

[es] U: qué fecha es hoy
     T: Hoy es domingo, 14 de marzo de 2027.          → answered · clock.date

[es] U: a cuántos estamos
     T: Estamos a 14 de marzo de 2027.                → answered · clock.date

[es] U: qué día es hoy
     T: Hoy es domingo, 14 de marzo de 2027.          → answered · clock.date

[es] U: fecha de hoy
     T: Hoy es domingo, 14 de marzo de 2027.          → answered · clock.date

[es] U: que dia es mañana
     T: Mañana es lunes, 15 de marzo de 2027.         → answered · clock.date · day=mañana

[es] U: qué fecha fue ayer
     T: Ayer fue sábado, 13 de marzo de 2027.         → answered · clock.date · day=ayer

[es] U: y dentro de 100 días qué fecha será
     T: Dentro de 100 días será martes, 22 de junio de 2027.
                                                      → answered · clock.date · day=+100 d

Given: now = 2027-03-14 18:42 America/New_York

[en] U: what's the date today
     T: Today is Sunday 14 March 2027.              → answered · clock.date

[en] U: what is today's date
     T: Today is Sunday 14 March 2027.              → answered · clock.date

[en] U: date
     T: Sunday 14 March 2027.                       → answered · clock.date

[en] U: what's tomorrow's date?
     T: Tomorrow is Monday 15 March 2027.           → answered · clock.date · day=tomorrow

[en] U: what was the date yesterday
     T: Yesterday was Saturday 13 March 2027.       → answered · clock.date · day=yesterday

[en] U: tell me the date please
     T: Today is Sunday 14 March 2027.              → answered · clock.date

[en] U: what's the date two weeks ago
     T: Two weeks ago it was Sunday 28 February 2027.
                                                      → answered · clock.date · day=-14 d
```

#### Edge cases

```text
Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: en qué año estamos
     T: Estamos en 2027.                              → answered · clock.date

[es] U: en qué mes estamos
     T: En marzo.                                     → answered · clock.date

[es] U: la fecha en formato iso
     T: 2027-03-14.                                   → answered · clock.date

Given: now = 2027-03-14 18:42 America/New_York
[en] U: what year is it
     T: It's 2027.                                    → answered · clock.date

[en] U: what month is it
     T: It's March.                                   → answered · clock.date

[en] U: today's date in ISO format
     T: 2027-03-14.                                   → answered · clock.date

Given: now = 2027-03-16 10:00 Europe/Madrid    (a Tuesday)
[es] U: qué fecha es el próximo viernes
     T: El próximo viernes es el 19 de marzo de 2027. → answered · clock.date · day=viernes 19-03-2027
     ("next Friday" resolves to the first Friday after today and says so)

[es] U: y el viernes de la semana que viene
     T: El viernes de la semana que viene es el 26 de marzo de 2027.
                                                      → answered · clock.date · day=viernes 26-03-2027
     (weeks run Monday to Sunday; this is the Friday of the following week)

Given: now = 2027-03-19 10:00 America/New_York    (a Friday)
[en] U: what's the date next Friday?
     T: Next Friday is 26 March 2027.                → answered · clock.date · day=2027-03-26
     (on a Friday, "next Friday" is a week away; "this Friday" would be today)

[en] U: what's the date this Friday
     T: This Friday is today, 19 March 2027.         → answered · clock.date · day=2027-03-19

[es] U: qué fecha es el 31 de abril
     T: El 31 de abril no existe.                     → answered · clock.date

[en] U: when is my birthday
     T: I don't know when your birthday is.           → unhandled
```

### clock.weekday

**Recognises** — the day of the week, today or on a date: "qué día de la
semana es hoy", "qué día cae el 25 de diciembre", "qué día de la semana fue el
20 de julio de 1969", "qué día era el 1/1/2000", "qué día será dentro de 3
semanas"; "what day is it", "what day of the week is today", "what day is
Christmas", "what weekday is 2 April 2027", "what day was July 20, 1969", "what
day will it be in 10 days".
**Slots** — `date` (datetime, optional, default today).
**Answers** — the weekday, with the date it resolved to, in the right tense.
For today the date is included: "Hoy es domingo (14 de marzo)". For a date
with no year, the next occurrence is used and the year is stated. Dates before
1583 are answered in the proleptic Gregorian calendar and the reply says so.
**Does not** —
- "qué fecha es hoy", "what's the date": `clock.date`.
- "cuántos días faltan para el viernes": `clock.until`.
- "qué día de la semana nací si nací el…": Talos does not know the user's
  birth date unless it is given in the message; with one in the message it is
  an ordinary date question.
- "qué día festivo es": holidays are not covered beyond the fixed list used by
  `clock.until`.

#### Examples

```text
Given: now = 2027-03-14 18:42 Europe/Madrid

[es] U: qué día de la semana es hoy
     T: Hoy es domingo (14 de marzo).                 → answered · clock.weekday

[es] U: qué día cae el 25 de diciembre
     T: El 25 de diciembre de 2027 es sábado.         → answered · clock.weekday · date=2027-12-25

[es] U: que dia de la semana fue el 20 de julio de 1969
     T: El 20 de julio de 1969 fue domingo.           → answered · clock.weekday · date=1969-07-20

[es] U: qué día era el 1/1/2000
     T: El 1 de enero de 2000 fue sábado.             → answered · clock.weekday · date=2000-01-01

[es] U: y qué día cae el 2 de abril
     T: El 2 de abril de 2027 es viernes.             → answered · clock.weekday · date=2027-04-02

[es] U: qué día será dentro de 3 semanas
     T: Dentro de 3 semanas será domingo, 4 de abril. → answered · clock.weekday · date=2027-04-04

Given: now = 2027-03-14 18:42 America/New_York

[en] U: what day is it
     T: It's Sunday (14 March).                       → answered · clock.weekday

[en] U: what day of the week is Christmas?
     T: 25 December 2027 is a Saturday.              → answered · clock.weekday · date=2027-12-25

[en] U: what day was July 20, 1969
     T: 20 July 1969 was a Sunday.                    → answered · clock.weekday · date=1969-07-20

[en] U: what weekday is 2 April 2027
     T: 2 April 2027 is a Friday.                     → answered · clock.weekday · date=2027-04-02

[en] U: what day will it be in 10 days
     T: In 10 days it will be Wednesday 24 March.     → answered · clock.weekday · date=2027-03-24

[en] U: day of the week tomorrow
     T: Tomorrow is Monday (15 March).                → answered · clock.weekday · date=2027-03-15
```

#### Edge cases

```text
Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: qué día cae el 29 de febrero de 2028
     T: El 29 de febrero de 2028 es martes.           → answered · clock.weekday · date=2028-02-29
     (leap day exists)

[es] U: qué día cae el 29 de febrero de 2027
     T: El 29 de febrero de 2027 no existe: 2027 no es bisiesto.
                                                      → answered · clock.weekday
     (nonexistent date, explained)

Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: que dia cae el 3/4/2027
     T: El 3 de abril de 2027 es sábado.              → answered · clock.weekday · date=2027-04-03
     (day first in Spanish: 3 April, not 4 March; the reply writes the date out)

Given: now = 2027-03-14 18:42 America/New_York
[en] U: what weekday is 3/4/2027
     T: Do you mean 3 April or 4 March?               → asks · choice   (clock.weekday)
[en] U: 4 March
     T: 4 March 2027 was a Thursday.                  → answered · clock.weekday · date=2027-03-04
     (numeric dates are ambiguous in English and asked; past tense because it is behind us)

[en] U: what day was January 1, 1500
     T: 1 January 1500 was a Monday in the proleptic Gregorian calendar.
                                                      → answered · clock.weekday · date=1500-01-01
     (before 1583 the reply names the calendar it used)

[en] U: what day is February 30
     T: 30 February doesn't exist.                    → answered · clock.weekday

[en] U: what day of the week was I born
     T: I don't know when you were born.              → unhandled · clock.weekday
     (no suggestion to supply the date: README, "Rules that apply everywhere")
```

### clock.until

**Recognises** — how long until, or since, a date, a time or a well-known day:
"cuánto falta para Navidad", "cuánto falta para las 7", "cuánto hace desde el 1
de enero de 2000", "cuántos días faltan para el viernes"; "how long until
Christmas", "how many days until July 4th", "how long until 9pm", "how long
since July 20, 1969", "days until Friday". Direction comes from the verb
("falta", "until" = forward; "hace", "desde", "since" = backward).
Well-known days, fixed list: Año Nuevo / New Year's Day (1 Jan), Reyes (6 Jan),
San Valentín / Valentine's Day (14 Feb), Halloween (31 Oct), Nochebuena /
Christmas Eve (24 Dec), Navidad / Christmas (25 Dec), Nochevieja / New Year's
Eve (31 Dec), and Easter Sunday (Domingo de Resurrección / Pascua, computed).
**Slots** — `target` (datetime, required).
**Answers** — the span, in the unit that fits:
- a date target is counted in calendar days ("Faltan 286 días");
- spans of a year or more are given as years, months and days;
- a time-of-day target is given in hours and minutes ("Faltan 2 horas y 18
  minutos");
- a time of day that has already passed today means tomorrow's, and the reply
  says "mañana"/"tomorrow";
- a bare hour without AM/PM means the soonest upcoming one ("para las 7" at
  18:42 is 19:00);
- the reply names the target with its resolved date when it came from a
  well-known day or a weekday.
A target that is already behind (explicit past year with "until") is answered
as past: "Eso fue hace …".
**Does not** —
- "cuántas horas de diferencia hay con Tokio": `timezones.difference`.
- "cuánto son 3 semanas en días": `calc.convert`.
- "avísame cuando falte una hora", timers, countdowns that notify: `unhandled`.
- "cuánto dura un vuelo", "how long is the movie": not a date computation;
  `unhandled`.

#### Examples

```text
Given: now = 2027-03-14 18:42 Europe/Madrid

[es] U: cuánto falta para navidad
     T: Faltan 286 días para Navidad (sábado, 25 de diciembre).
                                                      → answered · clock.until · target=2027-12-25

[es] U: cuanto falta para las 7
     T: Faltan 18 minutos.                            → answered · clock.until · target=2027-03-14 19:00

[es] U: dias que faltan para nochevieja
     T: Faltan 292 días para Nochevieja (viernes, 31 de diciembre).
                                                      → answered · clock.until · target=2027-12-31

[es] U: cuanto falta para medianoche
     T: Faltan 5 horas y 18 minutos.                  → answered · clock.until · target=2027-03-15 00:00

[es] U: cuánto hace desde el 1 de enero de 2000
     T: Han pasado 27 años, 2 meses y 13 días.        → answered · clock.until · target=2000-01-01

[es] U: cuántos días faltan para el viernes
     T: Faltan 5 días (viernes, 19 de marzo).         → answered · clock.until · target=2027-03-19

[es] U: cuanto queda para pascua
     T: Faltan 14 días para el Domingo de Resurrección (28 de marzo).
                                                      → answered · clock.until · target=2027-03-28

Given: now = 2027-03-14 18:42 America/New_York

[en] U: how long until Christmas?
     T: 286 days until Christmas (Saturday 25 December).
                                                      → answered · clock.until · target=2027-12-25

[en] U: how long until 9pm
     T: 2 hours and 18 minutes.                       → answered · clock.until · target=2027-03-14 21:00

[en] U: how many days until July 4th
     T: 112 days until 4 July (Sunday).               → answered · clock.until · target=2027-07-04

[en] U: how long until midnight
     T: 5 hours and 18 minutes.                       → answered · clock.until · target=2027-03-15 00:00

[en] U: how long since July 20, 1969
     T: 57 years, 7 months and 22 days.               → answered · clock.until · target=1969-07-20

[en] U: days until Friday
     T: 5 days (Friday 19 March).                     → answered · clock.until · target=2027-03-19

[en] U: how long until Easter
     T: 14 days until Easter Sunday (28 March).       → answered · clock.until · target=2027-03-28
```

#### Edge cases

```text
Given: now = 2027-03-14 18:42 Europe/Madrid
[es] U: cuánto falta para las 18:00
     T: Para las 18:00 de mañana faltan 23 horas y 18 minutos.
                                                      → answered · clock.until · target=2027-03-15 18:00
     (a time already passed today means tomorrow, and the reply says so)

[es] U: cuánto falta para el domingo
     T: Hoy es domingo. El próximo domingo es dentro de 7 días (21 de marzo).
                                                      → answered · clock.until · target=2027-03-21
     (a weekday that is today: zero would be useless, so the reply says it is today and gives the next)

[es] U: cuánto falta para mañana
     T: Falta 1 día (lunes, 15 de marzo).             → answered · clock.until · target=2027-03-15

[es] U: cuánto falta para el 30
     T: Faltan 16 días (martes, 30 de marzo).         → answered · clock.until · target=2027-03-30
     (a day number alone: the next such day of the month)

[es] U: cuánto falta para el 3/4
     T: Faltan 20 días para el 3 de abril de 2027.    → answered · clock.until · target=2027-04-03
     (day first in Spanish; the reply writes the date out)

[es] U: cuánto falta para el 31 de abril
     T: El 31 de abril no existe.                     → answered · clock.until

[es] U: cuánto falta para mi cumpleaños
     T: ¿Qué fecha es tu cumpleaños?                  → asks · target
[es] U: el 9 de agosto
     T: Faltan 148 días (lunes, 9 de agosto).         → answered · clock.until · target=2027-08-09

Given: now = 2027-03-14 18:42 America/New_York
[en] U: how long until 3/4
     T: Do you mean 3 April or 4 March?               → asks · choice   (clock.until)
[en] U: the 4th of March
     T: 356 days until 4 March 2028.                  → answered · clock.until · target=2028-03-04
     (numeric dates are asked in English; 4 March has passed, so "until" takes next year's)

[en] U: how long since 4 March
     T: 10 days since 4 March 2027.                   → answered · clock.until · target=2027-03-04
     (the same date with "since" takes the most recent one)

Given: now = 2027-03-16 10:00 America/New_York    (a Tuesday)
[en] U: how long until next Friday
     T: 3 days until Friday 19 March.                 → answered · clock.until · target=2027-03-19
     (first Friday after today; the date is stated so a different reading can be corrected)

[en] U: how long until Friday next week
     T: 10 days until Friday 26 March.                → answered · clock.until · target=2027-03-26

[en] U: how long until July 20, 1969
     T: That was 57 years, 7 months and 22 days ago.
                                                      → answered · clock.until · target=1969-07-20
     (a past date with "until" is reported as past)

[en] U: how long until
     T: Until what?                                   → asks · target

[en] U: how long is the flight to Lisbon
     T: I can't tell that.                            → unhandled
```

---

## calc

Arithmetic, percentages and unit conversion. Calculation uses exact decimals
and fractions, never binary floating point, and rounds only when displaying.
Numbers follow the conventions at the top of this file.

### calc.arithmetic

**Recognises** — an expression to evaluate, written with symbols or words, with
or without a lead-in ("cuánto es", "calcula", "cuánto da", "what is",
"calculate", "=?"). Operators: `+`, `-`, `*`, `x`, `×`, `/`, `÷`, `^`, `**`,
parentheses. Words: "más", "menos", "por", "entre", "dividido por", "elevado
a", "al cuadrado", "al cubo", "raíz cuadrada de", "raíz cúbica de"; "plus",
"minus", "times", "multiplied by", "divided by", "over", "to the power of",
"squared", "cubed", "square root of", "cube root of", "sqrt". Number words as
well as digits. Typographic variants (`×`, `−`, `÷`) are normalised before
anything else.
**Slots** — `expression` (term, required).
**Answers** — the expression echoed in normalised form and its value. Usual
precedence: parentheses, then powers (right to left), then multiplication and
division (left to right), then addition and subtraction; a unary minus binds
looser than a power (`-2^2` is −4). Integers are shown exactly up to 30 digits;
above that, and for anything that is not an integer, the value is shown with
at most 10 significant digits. Exact fractions that do not terminate are shown
rounded with `≈` (10/3 is `≈ 3,333333333`). Values with more than 30 digits
are shown in scientific notation with `≈`. A result beyond 10^1000, or an
input number with more than 100 digits, is too large and is refused.
**Does not** —
- Any expression with `%`, or "por ciento"/"percent": `calc.percent`
  ("50 + 10%" is a percentage question).
- "5 km a millas": `calc.convert`.
- "resuelve 2x+3=7", "derivada de x²": algebra and calculus; `unhandled`.
- "resto de 17 entre 5", factorials, trigonometry, constants such as pi: not
  supported; `unhandled`.
- Two expressions in one message: only the first is evaluated.
- "2+2 es 5?": a claim to check, evaluated as the expression `2+2`; Talos
  gives the value and does not judge the user.

#### Examples

```text
[es] U: cuánto es 2+2
     T: 2 + 2 = 4.                                    → answered · calc.arithmetic · expression=2+2

[es] U: (3 + 4) * 5
     T: (3 + 4) × 5 = 35.                             → answered · calc.arithmetic · expression=(3+4)*5

[es] U: tres por cuatro
     T: 3 × 4 = 12.                                   → answered · calc.arithmetic · expression=3*4

[es] U: calcula 2 + 3 * 4
     T: 2 + 3 × 4 = 14.                               → answered · calc.arithmetic · expression=2+3*4
     (multiplication before addition)

[es] U: raiz cuadrada de 144
     T: √144 = 12.                                    → answered · calc.arithmetic · expression=sqrt(144)

[es] U: 2 elevado a 10
     T: 2^10 = 1024.                                  → answered · calc.arithmetic · expression=2^10

[es] U: cuanto da 10 entre 4
     T: 10 ÷ 4 = 2,5.                                 → answered · calc.arithmetic · expression=10/4

[es] U: dos mil quinientos mas trescientos
     T: 2500 + 300 = 2800.                            → answered · calc.arithmetic · expression=2500+300

[en] U: what's 12 times 12
     T: 12 × 12 = 144.                                → answered · calc.arithmetic · expression=12*12

[en] U: 7 + 8 * 2
     T: 7 + 8 × 2 = 23.                               → answered · calc.arithmetic · expression=7+8*2

[en] U: three times four
     T: 3 × 4 = 12.                                   → answered · calc.arithmetic · expression=3*4

[en] U: 100 divided by 8
     T: 100 ÷ 8 = 12.5.                               → answered · calc.arithmetic · expression=100/8

[en] U: sqrt(81)
     T: √81 = 9.                                      → answered · calc.arithmetic · expression=sqrt(81)

[en] U: 2 to the power of 8
     T: 2^8 = 256.                                    → answered · calc.arithmetic · expression=2^8

[en] U: calculate (1,250 + 750) / 4
     T: (1,250 + 750) ÷ 4 = 500.                      → answered · calc.arithmetic · expression=(1250+750)/4

[en] U: what is 9 squared
     T: 9² = 81.                                      → answered · calc.arithmetic · expression=9^2
```

#### Edge cases

```text
[es] U: 5 / 0
     T: No se puede dividir entre cero.               → answered · calc.arithmetic

[en] U: 0/0
     T: Zero divided by zero is undefined.            → answered · calc.arithmetic

[en] U: what's 10 / 3
     T: 10 ÷ 3 ≈ 3.333333333.                         → answered · calc.arithmetic
     (non-terminating: rounded to 10 significant digits and marked ≈)

[es] U: 1/3*3
     T: 1 ÷ 3 × 3 = 1.                                → answered · calc.arithmetic
     (exact fractions: no 0,9999… artefact)

[es] U: 0,1 + 0,2
     T: 0,1 + 0,2 = 0,3.                              → answered · calc.arithmetic
     (exact decimals, not binary floating point)

[en] U: 0.1 + 0.2
     T: 0.1 + 0.2 = 0.3.                              → answered · calc.arithmetic

[es] U: calcula 3,5 x 2
     T: 3,5 × 2 = 7.                                  → answered · calc.arithmetic
     (decimal comma in Spanish; "x" between numbers is multiplication)

[es] U: 3.5 + 1
     T: 3,5 + 1 = 4,5.                                → answered · calc.arithmetic
     (a decimal point is accepted in Spanish too, and the reply answers in Spanish format)

[es] U: 1.000 + 1
     T: 1.000 + 1 = 1001 (he leído 1.000 como mil).   → answered · calc.arithmetic
     (a lone dot followed by three digits is a thousands separator in Spanish; the reading is shown)

[es] U: 1,000 + 1
     T: 1,000 + 1 = 2 (he leído 1,000 como uno).      → answered · calc.arithmetic
     (in Spanish the comma is always the decimal mark; the reading is shown)

[en] U: 1,000 + 1
     T: 1,000 + 1 = 1,001.                            → answered · calc.arithmetic
     (in English the comma followed by three digits is a thousands separator)

[en] U: 1,5 + 1
     T: 1,5 + 1 = 2.5 (I read 1,5 as 1.5).            → answered · calc.arithmetic
     (a comma not followed by exactly three digits cannot be a separator; the reading is shown)

[es] U: 1.000,50 + 1
     T: 1.000,50 + 1 = 1001,5.                        → answered · calc.arithmetic
     (when both marks appear, the last one is the decimal mark)

[en] U: -2^2
     T: −2² = −4.                                     → answered · calc.arithmetic
     (a power binds tighter than a unary minus)

[en] U: (-2)^2
     T: (−2)² = 4.                                    → answered · calc.arithmetic

[en] U: 2^3^2
     T: 2^3^2 = 512.                                  → answered · calc.arithmetic
     (powers associate to the right)

[es] U: 99999999999 * 99999999999
     T: 99999999999 × 99999999999 = 9.999.999.999.800.000.000.001.
                                                      → answered · calc.arithmetic
     (22 digits: shown exactly)

[en] U: 2^64
     T: 2^64 = 18,446,744,073,709,551,616.            → answered · calc.arithmetic

[en] U: 2^200
     T: 2^200 ≈ 1.606938044 × 10^60.                  → answered · calc.arithmetic
     (more than 30 digits: scientific notation)

[es] U: 9^9^9
     T: El resultado es demasiado grande para calcularlo. → unhandled · calc.arithmetic
     (beyond 10^1000)

[en] U: 10^400
     T: 10^400 = 1 × 10^400.                          → answered · calc.arithmetic

[es] U: raíz de -4
     T: -4 no tiene raíz cuadrada real.               → answered · calc.arithmetic

[en] U: square root of 2
     T: √2 ≈ 1.414213562.                             → answered · calc.arithmetic

[es] U: un billón entre mil
     T: 1.000.000.000.000 ÷ 1000 = 1.000.000.000.     → answered · calc.arithmetic
     (Spanish "billón" is 10^12)

[en] U: one billion divided by a thousand
     T: 1,000,000,000 ÷ 1,000 = 1,000,000.            → answered · calc.arithmetic
     (English "billion" is 10^9)

[es] U: veintiuno por tres
     T: 21 × 3 = 63.                                  → answered · calc.arithmetic

[en] U: twenty-one times three
     T: 21 × 3 = 63.                                  → answered · calc.arithmetic

[en] U: 8 / 2(2+2)
     T: Write the multiplication with a sign: 2 × (2 + 2).  → asks · expression
     (implicit multiplication is not guessed)

[es] U: calcula
     T: ¿Qué quieres calcular?                        → asks · expression
[es] U: 12 por 12
     T: 12 × 12 = 144.                                → answered · calc.arithmetic · expression=12*12

[en] U: 2 +
     T: That expression is incomplete. What should I add to 2?  → asks · expression

[es] U: resuelve 2x+3=7
     T: No resuelvo ecuaciones; solo hago cuentas con números. → unhandled

[en] U: 50 + 10%
     T: (answered by calc.percent: 50 plus 10% is 55)  → answered · calc.percent

[en] U: 2+2 is 5, right?
     T: 2 + 2 = 4.                                    → answered · calc.arithmetic
```

### calc.percent

**Recognises** — four shapes of percentage question, plus phrasings of each:
- *a percentage of an amount*: "el 15% de 80", "15 por ciento de 200", "what is
  15% of 80", "12.5 percent of 80";
- *what percentage one is of another*: "qué porcentaje es 20 de 50", "what
  percent is 20 of 50", "what percentage of 200 is 50";
- *raising or lowering an amount by a percentage*: "80 más un 15%", "sube 80
  un 15%", "50 euros con un 20% de descuento", "increase 80 by 15%", "$50 with
  20% off", "80 + 15%", "100 más el 21% de IVA";
- *the change between two amounts*: "de 50 a 65 cuánto ha subido en porcentaje",
  "percent change from 50 to 65";
- *the whole, given a part*: "20 es el 40% de qué número", "20 is 40% of what".
- tips: "la propina del 10% de 45", "20% tip on 45".
**Slots** — `percent` (number), `amount` (number), `other` (number): which are
needed depends on the shape. A missing one is asked for. A currency symbol or a
unit attached to the amount is carried over to the result.
**Answers** — the result of the one calculation, echoed, with the percentage
sign written as the language writes it (`40 %` in Spanish, `40%` in English).
Results are shown with at most two decimals, trailing zeros dropped, and `≈`
when rounded. A tip question gives the tip and the total. A change between two
amounts says whether it is an increase or a decrease.
**Does not** —
- "50 + 10" and any expression without a percent sign or words:
  `calc.arithmetic`.
- "qué es un porcentaje": `define.word`.
- "interés compuesto", "hipoteca", "IVA de mi factura" without numbers: not
  supported; `unhandled`, or `asks` when only a number is missing.
- "añade el IVA a 100" with no rate: Talos does not assume a rate; it asks.

#### Examples

```text
[es] U: el 15% de 80
     T: El 15 % de 80 es 12.                          → answered · calc.percent · percent=15 amount=80

[es] U: qué porcentaje es 20 de 50
     T: 20 es el 40 % de 50.                          → answered · calc.percent · part=20 whole=50

[es] U: 80 mas un 15%
     T: 80 más un 15 % es 92.                         → answered · calc.percent · amount=80 percent=+15

[es] U: 50 euros con un 20% de descuento
     T: Con un 20 % de descuento, 50 € se quedan en 40 €.
                                                      → answered · calc.percent · amount=50 percent=-20

[es] U: de 50 a 65 cuánto ha subido en porcentaje
     T: Ha subido un 30 %.                            → answered · calc.percent · from=50 to=65

[es] U: 20 es el 40% de qué numero
     T: 20 es el 40 % de 50.                          → answered · calc.percent · part=20 percent=40

[es] U: la propina del 10% de 45
     T: La propina es 4,5; en total, 49,5.            → answered · calc.percent · percent=10 amount=45

[es] U: el 12,5% de 80
     T: El 12,5 % de 80 es 10.                        → answered · calc.percent · percent=12.5 amount=80

[en] U: what is 15% of 80
     T: 15% of 80 is 12.                              → answered · calc.percent · percent=15 amount=80

[en] U: what percent is 20 of 50
     T: 20 is 40% of 50.                              → answered · calc.percent · part=20 whole=50

[en] U: increase 80 by 15%
     T: 80 plus 15% is 92.                            → answered · calc.percent · amount=80 percent=+15

[en] U: $50 with 20% off
     T: $50 with 20% off is $40.                      → answered · calc.percent · amount=50 percent=-20

[en] U: percent change from 50 to 65
     T: That's a 30% increase.                        → answered · calc.percent · from=50 to=65

[en] U: 20 is 40% of what
     T: 20 is 40% of 50.                              → answered · calc.percent · part=20 percent=40

[en] U: 20% tip on 45
     T: The tip is 9; the total is 54.                → answered · calc.percent · percent=20 amount=45

[en] U: what percentage of 200 is 50?
     T: 50 is 25% of 200.                             → answered · calc.percent · part=50 whole=200
```

#### Edge cases

```text
[es] U: el 0% de 80
     T: El 0 % de 80 es 0.                            → answered · calc.percent

[en] U: 150% of 80
     T: 150% of 80 is 120.                            → answered · calc.percent

[es] U: el 15% de -80
     T: El 15 % de -80 es -12.                        → answered · calc.percent

[en] U: what percent is 5 of 0
     T: A percentage of zero is undefined.            → answered · calc.percent

[es] U: de 0 a 5 cuánto ha subido en porcentaje
     T: Subir desde 0 no se puede expresar en porcentaje. → answered · calc.percent

[en] U: percent change from 80 to 60
     T: That's a 25% decrease.                        → answered · calc.percent

[en] U: decrease 100 by 100%
     T: 100 minus 100% is 0.                          → answered · calc.percent

[es] U: el 15% de 99,99
     T: El 15 % de 99,99 es ≈ 15.                     → answered · calc.percent
     (14,9985 rounded to two decimals is 15,00, shown as ≈ 15)

[es] U: descuento del 30% sobre 59,90
     T: Con un 30 % de descuento, 59,90 se queda en 41,93.
                                                      → answered · calc.percent

[es] U: 100 más el 21% de IVA
     T: 100 más un 21 % de IVA es 121.                → answered · calc.percent
     (the user gave the rate)

[es] U: añade el iva a 100
     T: ¿Qué porcentaje de IVA?                       → asks · percent
[es] U: 21
     T: 100 más un 21 % de IVA es 121.                → answered · calc.percent · amount=100 percent=+21

[en] U: 80 + 15%
     T: 80 plus 15% is 92.                            → answered · calc.percent
     (a trailing percent after "+" is read as an increase, and the reply says it is)

[en] U: what's 50% of
     T: 50% of what number?                           → asks · amount

[es] U: interés compuesto de 1000 al 3% durante 5 años
     T: No calculo intereses compuestos.              → unhandled
```

### calc.convert

**Recognises** — turning a quantity from one unit into another, in either word
order: "5 km a millas", "convierte 100 grados fahrenheit a celsius", "pasa 2 GB
a MB", "cuántos litros son 3 galones", "una libra en gramos", "cuántos segundos
hay en una hora"; "convert 10 miles to km", "72 F to C", "how many ounces in a
pound", "5 feet in cm", "how many seconds in a day".
Categories and units:
- length: mm, cm, m, km, in, ft, yd, mi, nautical mile;
- mass: mg, g, kg, metric tonne, oz, lb;
- temperature: °C, °F, K;
- volume: ml, cl, l, US fl oz, US gallon;
- time: s, min, h, day, week, month, year;
- data size: bit, byte, kB, MB, GB, TB (decimal, powers of 1000) and KiB, MiB,
  GiB, TiB (binary, powers of 1024);
- speed: m/s, km/h, mph, knot.
Everyday names and abbreviations in both languages are accepted ("megas",
"pulgadas", "millas", "libras", "feet").
**Slots** — `amount` (number, optional, default 1: "una libra en gramos"),
`from` (term, required), `to` (term, required). When `to` is missing and the
user has remembered units ([state.md](state.md)), the target is the usual
counterpart of `from` in that system (miles → km for metric, km → miles for
imperial); otherwise, or when `from` is already in that system, `to` is asked.
**Answers** — the converted value with its unit and the converted-from quantity
echoed. Conversions that are exact by definition (inches to centimetres,
kilometres to metres, Celsius to Kelvin) are shown exactly. Others are shown
rounded to two decimals, trailing zeros dropped, with `≈`. A nonzero result
that would round to zero keeps up to four significant digits instead. Gallons
and fluid ounces are the US ones, and the reply says so. Data sizes use
decimal prefixes by default and name the binary equivalent when it differs
meaningfully. Months and years convert to and from each other exactly (12
months) and to days using the average calendar lengths (365.2425 days to a
year), marked `≈` and "de media"/"on average". A bare "degrees" is the one of
the three temperature units that is not the target ("20 grados a fahrenheit"
is Celsius; "70 degrees to celsius" is Fahrenheit); with no target it is asked.
**Does not** —
- currency ("100 euros a dólares"): there are no exchange rates; `unhandled`,
  said plainly.
- area, pressure, energy, power, fuel economy, data rates ("Mbps a MB/s"): not
  supported; `unhandled`, naming the categories that are.
- "qué hora es en Tokio" and "3 pm Madrid in Tokyo": `timezones.time_in`.
- "cuánto falta para el viernes": `clock.until`.
- "cuánto son 5 más 3": `calc.arithmetic`.
- "cómo se dice litro en inglés": `translate.word`.

#### Examples

```text
[es] U: 5 km a millas
     T: 5 km son ≈ 3,11 mi.                           → answered · calc.convert · amount=5 from=km to=mi

[es] U: convierte 100 grados fahrenheit a celsius
     T: 100 °F son ≈ 37,78 °C.                        → answered · calc.convert · amount=100 from=°F to=°C

[es] U: cuantos litros son 3 galones
     T: 3 galones (EE. UU.) son ≈ 11,36 l.            → answered · calc.convert · amount=3 from=gal to=l

[es] U: pasa 2 gb a mb
     T: 2 GB son 2000 MB (2 GiB son 2048 MiB).        → answered · calc.convert · amount=2 from=GB to=MB

[es] U: cuántos segundos hay en una hora
     T: 1 hora son 3600 segundos.                     → answered · calc.convert · amount=1 from=h to=s

[es] U: 100 km/h en mph
     T: 100 km/h son ≈ 62,14 mph.                     → answered · calc.convert · amount=100 from=km/h to=mph

[es] U: una libra en gramos
     T: 1 libra son ≈ 453,59 g.                       → answered · calc.convert · amount=1 from=lb to=g

[es] U: cuantos cm son 5 pulgadas
     T: 5 pulgadas son 12,7 cm.                       → answered · calc.convert · amount=5 from=in to=cm

[en] U: convert 10 miles to km
     T: 10 mi is ≈ 16.09 km.                          → answered · calc.convert · amount=10 from=mi to=km

[en] U: 72 f to c
     T: 72 °F is ≈ 22.22 °C.                          → answered · calc.convert · amount=72 from=°F to=°C

[en] U: how many ounces in a pound
     T: 1 lb is 16 oz.                                → answered · calc.convert · amount=1 from=lb to=oz

[en] U: 5 feet in cm
     T: 5 ft is 152.4 cm.                             → answered · calc.convert · amount=5 from=ft to=cm

[en] U: 3 gallons in liters
     T: 3 US gal is ≈ 11.36 L.                        → answered · calc.convert · amount=3 from=gal to=l

[en] U: how many seconds in a day
     T: 1 day is 86,400 seconds.                      → answered · calc.convert · amount=1 from=day to=s

[en] U: 60 mph to km/h
     T: 60 mph is ≈ 96.56 km/h.                       → answered · calc.convert · amount=60 from=mph to=km/h

[en] U: 25 c to f
     T: 25 °C is 77 °F.                               → answered · calc.convert · amount=25 from=°C to=°F
```

#### Edge cases

```text
[es] U: convierte 20 grados a fahrenheit
     T: 20 °C son 68 °F (he entendido grados Celsius). → answered · calc.convert · from=°C to=°F
     (a bare "grados" is the temperature unit that is not the target)

[en] U: 70 degrees to celsius
     T: 70 °F is ≈ 21.11 °C (I took degrees as Fahrenheit).
                                                      → answered · calc.convert
[es] U: convierte 20 grados
     T: ¿A qué unidad, Celsius, Fahrenheit o Kelvin?  → asks · to

[en] U: convert 5 km
     T: Convert it to what?                           → asks · to
[en] U: miles
     T: 5 km is ≈ 3.11 mi.                            → answered · calc.convert · amount=5 from=km to=mi

[es] U: 0 grados celsius a kelvin
     T: 0 °C son 273,15 K.                            → answered · calc.convert

[en] U: -300 c to k
     T: -300 °C is below absolute zero, so it has no Kelvin value. → answered · calc.convert

[en] U: -40 c to f
     T: -40 °C is -40 °F.                             → answered · calc.convert

[es] U: 5 kg a metros
     T: Los kilos son masa y los metros longitud: no se pueden convertir. → answered · calc.convert

[en] U: convert 3 liters to hours
     T: Liters are volume and hours are time, so I can't convert between them.
                                                      → answered · calc.convert

[es] U: 5 flurbos a metros
     T: No conozco la unidad «flurbos».               → unhandled · calc.convert

[es] U: 100 euros a dolares
     T: No convierto monedas: no tengo cotizaciones.  → unhandled · calc.convert

[en] U: 5 m2 to ft2
     T: I don't convert areas. I handle length, mass, temperature, volume, time, data size and speed.
                                                      → unhandled · calc.convert

[en] U: 100 Mbps to MB/s
     T: I don't convert data rates. I handle data sizes, but not per-second rates.
                                                      → unhandled · calc.convert

[es] U: 2,5 km a m
     T: 2,5 km son 2500 m.                            → answered · calc.convert
     (decimal comma)

[es] U: 1.000 m a km
     T: 1.000 m son 1 km (he leído 1.000 como mil).   → answered · calc.convert
     (Spanish thousands dot; the reading is shown)

[en] U: 1,000 m to km
     T: 1,000 m is 1 km.                              → answered · calc.convert

[en] U: 2.5 km to m
     T: 2.5 km is 2,500 m.                            → answered · calc.convert

[es] U: 8 Mb a MB
     T: 8 Mb (megabits) son 1 MB (megabytes).         → answered · calc.convert
     (a capital M with a lower-case b is bits, B is bytes)

[en] U: 1 gb in mb
     T: 1 GB is 1,000 MB (1 GiB is 1,024 MiB).        → answered · calc.convert
     (decimal by default, binary pair named)

[es] U: 5 km a km
     T: 5 km son 5 km.                                → answered · calc.convert
     (same unit: still answered)

[es] U: 1 mm a km
     T: 1 mm son 0,000001 km.                         → answered · calc.convert
     (exact and tiny: not rounded to zero and not marked ≈)

[es] U: un millón de km a mm
     T: 1.000.000 km son 1.000.000.000.000 mm.        → answered · calc.convert

[en] U: 1e30 km to mm
     T: 1 × 10^30 km is 1 × 10^36 mm.                 → answered · calc.convert
     (beyond 30 digits: scientific)

[es] U: cuántos metros son 5 millas náuticas
     T: 5 millas náuticas son 9260 m.                 → answered · calc.convert

[es] U: cuántos días son 2 años
     T: 2 años son ≈ 730,49 días (de media).          → answered · calc.convert
     (years have no fixed length in days; the average is used and said)

[en] U: 100 f to k
     T: 100 °F is ≈ 310.93 K.                         → answered · calc.convert

[en] U: what is 3 times 4 in miles
     T: (not a conversion)                            → answered · calc.arithmetic · expression=3*4
     (the arithmetic is answered; "in miles" has nothing to convert from)
```

---

## random

Dice, coins, choosing among options, and random numbers. Every skill draws
from the source described under "Randomness" at the top of this file:
varied from turn to turn, and reproducible on replay. The reply
states the outcome plainly and never comments on luck.

### random.dice

**Recognises** — rolling dice, in notation or in words: "tira un dado", "2d6",
"3d8+2", "lanza dos dados", "tira 4 dados de 6 caras", "tírame un d20", "tira
los dados"; "roll a die", "roll 2d6", "roll two dice", "d20", "throw 3d8+2",
"roll 4 six-sided dice", "roll the dice".
Notation is `NdM` with an optional whole-number modifier (`+K` or `-K`). Defaults:
"un dado"/"a die" is one six-sided die; plural "los dados"/"the dice" with no
number is two six-sided dice, and the reply says "dos dados"/"two dice".
**Slots** — `count` (number, default 1, 1 to 100), `sides` (number, default 6, 2
to 1000), `modifier` (number, default 0).
**Answers** — for one die, the face. For several, the individual faces and the
total (with the modifier shown) when there are ten or fewer; above ten, the
total only. Each die consumes one draw, in order.
**Does not** —
- "un número del 1 al 6": `random.number`.
- "tira una moneda": `random.coin`.
- "tira un dado para decidir entre A y B": the options make it `random.pick`.
- "4d6 quedándote con los 3 mejores", mixed dice such as "d6 + d20", exploding
  or advantage rolls: not supported; `unhandled`, said plainly.
- "juega a los dados": a game, not specified yet.

#### Examples

```text
Given: draw = 4
[es] U: tira un dado
     T: Ha salido un 4.                               → answered · random.dice · count=1 sides=6

Given: draw = 3, 5
[es] U: 2d6
     T: 3 y 5: total 8.                               → answered · random.dice · count=2 sides=6

Given: draw = 6, 1
[es] U: lanza dos dados
     T: 6 y 1: total 7.                               → answered · random.dice · count=2 sides=6

Given: draw = 17
[es] U: tirame un d20
     T: Ha salido un 17.                              → answered · random.dice · count=1 sides=20

Given: draw = 5, 2, 8
[es] U: tira 3d8+2
     T: 5, 2 y 8, más 2: total 17.                    → answered · random.dice · count=3 sides=8 modifier=2

Given: draw = 1, 1, 6, 3
[es] U: tira 4 dados de 6 caras
     T: 1, 1, 6 y 3: total 11.                        → answered · random.dice · count=4 sides=6

Given: draw = 2, 2
[es] U: tira los dados
     T: Dos dados: 2 y 2, total 4.                    → answered · random.dice · count=2 sides=6

Given: draw = 4
[en] U: roll a die
     T: It came up 4.                                 → answered · random.dice · count=1 sides=6

Given: draw = 3, 5
[en] U: roll 2d6
     T: 3 and 5: total 8.                             → answered · random.dice · count=2 sides=6

Given: draw = 6, 1
[en] U: roll two dice
     T: 6 and 1: total 7.                             → answered · random.dice · count=2 sides=6

Given: draw = 17
[en] U: d20
     T: 17.                                           → answered · random.dice · count=1 sides=20

Given: draw = 5, 2, 8
[en] U: throw 3d8+2
     T: 5, 2 and 8, plus 2: total 17.                 → answered · random.dice · count=3 sides=8 modifier=2

Given: draw = 1, 1, 6, 3
[en] U: roll 4 six-sided dice
     T: 1, 1, 6 and 3: total 11.                      → answered · random.dice · count=4 sides=6

Given: draw = 2, 2
[en] U: roll the dice
     T: Two dice: 2 and 2, total 4.                   → answered · random.dice · count=2 sides=6
```

#### Edge cases

```text
[es] U: tira 0d6
     T: Tengo que tirar al menos un dado.             → unhandled · random.dice
     (zero dice)

[en] U: roll a d1
     T: A die needs at least 2 sides.                 → unhandled · random.dice

[en] U: roll 101d6
     T: I can roll up to 100 dice at once.            → unhandled · random.dice

[es] U: tira un d1000000
     T: Los dados pueden tener como mucho 1000 caras. → unhandled · random.dice

Given: draw = 5
[es] U: tira un dado de 7 caras
     T: Ha salido un 5.                               → answered · random.dice · count=1 sides=7
     (any side count from 2 to 1000)

Given: draw = 2
[en] U: roll 1d6-10
     T: It came up 2, minus 10: total -8.             → answered · random.dice · modifier=-10
     (the total may be negative)

Given: draw = 3, 5, 2, 6, 1, 4, 4, 2, 6, 5, 1, 3
[en] U: roll 12d6
     T: Total 42.                                     → answered · random.dice · count=12 sides=6
     (more than ten dice: the total alone)

[en] U: roll 4d6 and keep the highest 3
     T: I only roll plain dice such as 4d6 or 2d8+1.  → unhandled · random.dice

[en] U: roll a d6 and a d20
     T: Ask me for one kind of die at a time.         → unhandled · random.dice

[es] U: 2D6 + 3
     T: (uppercase D and spaces around the modifier are accepted) → answered · random.dice · modifier=3

[es] U: tira un dado para decidir entre ir al cine o cenar
     T: Elijo cenar.                                  → answered · random.pick
     (the options decide the intent; given draw = option 2)

[en] U: pick a number from 1 to 6
     T: 4.                                            → answered · random.number
     (a range of numbers is not a die)
```

### random.coin

**Recognises** — flipping a coin: "tira una moneda", "cara o cruz", "echa una
moneda al aire", "lanza una moneda", "tira 3 monedas"; "flip a coin", "heads or
tails", "toss a coin", "coin flip", "flip 3 coins". A call ("yo cara", "I call
heads") is ignored and the coin is flipped anyway.
**Slots** — `count` (number, default 1, 1 to 100).
**Answers** — for one coin, "cara"/"cruz" or "heads"/"tails". For several, the
sequence when there are ten or fewer; above ten, the counts of each. A coin
never lands on its edge. Each coin consumes one draw.
**Does not** —
- "tira una moneda para decidir entre pizza o sushi", "flip a coin to choose
  between tea and coffee": the options make it `random.pick`.
- "tira un dado": `random.dice`.
- "cuánto cuesta una moneda de 2 euros", "how many cents in a coin": not a
  flip.
- "cara o cruz" as a two-option decision with no options named is a flip.

#### Examples

```text
Given: draw = heads
[es] U: tira una moneda
     T: Ha salido cara.                               → answered · random.coin · count=1

Given: draw = tails
[es] U: cara o cruz
     T: Cruz.                                         → answered · random.coin · count=1

Given: draw = heads
[es] U: echa una moneda al aire
     T: Cara.                                         → answered · random.coin · count=1

Given: draw = tails
[es] U: lanza una moneda porfa
     T: Ha salido cruz.                               → answered · random.coin · count=1

Given: draw = heads, tails, tails
[es] U: tira 3 monedas
     T: Cara, cruz, cruz.                             → answered · random.coin · count=3

Given: draw = tails
[es] U: moneda
     T: Cruz.                                         → answered · random.coin · count=1

Given: draw = heads
[en] U: flip a coin
     T: Heads.                                        → answered · random.coin · count=1

Given: draw = tails
[en] U: heads or tails?
     T: Tails.                                        → answered · random.coin · count=1

Given: draw = heads
[en] U: toss a coin
     T: It landed heads.                              → answered · random.coin · count=1

Given: draw = tails
[en] U: coin flip
     T: Tails.                                        → answered · random.coin · count=1

Given: draw = heads, tails, tails
[en] U: flip 3 coins
     T: Heads, tails, tails.                          → answered · random.coin · count=3

Given: draw = heads
[en] U: flip a coin please
     T: Heads.                                        → answered · random.coin · count=1
```

#### Edge cases

```text
Given: draw = tails
[en] U: I call heads. flip a coin
     T: Tails.                                        → answered · random.coin
     (the call is ignored; the reply does not say who won)

Given: draw = tails
[es] U: yo cara. tira una moneda
     T: Cruz.                                         → answered · random.coin

Given: draw = 12 heads and 8 tails over 20 draws
[en] U: flip 20 coins
     T: 12 heads and 8 tails.                         → answered · random.coin · count=20
     (more than ten: counts, not the sequence)

[es] U: tira 500 monedas
     T: Como mucho tiro 100 monedas a la vez.         → unhandled · random.coin

Given: draw = option 2
[es] U: tira una moneda para decidir entre pizza o sushi
     T: Elijo sushi.                                  → answered · random.pick
     (options named: a pick, not a flip)

Given: draw = option 1
[en] U: flip a coin to choose between tea and coffee
     T: Tea.                                          → answered · random.pick

[es] U: cuánto vale una moneda de 2 euros
     T: No tengo ese dato.                            → unhandled
     (a question about coins, not a flip)
```

### random.pick

**Recognises** — choosing one of several options the user gives, with a verb of
choosing: "elige entre pizza, sushi y tacos", "escoge uno: rojo, verde, azul",
"decide por mí: cine o casa", "sortea a Ana, Luis y Marta", "ayúdame a elegir
entre pizza o sushi"; "pick one: red, green, blue", "choose between tea and
coffee", "decide for me: stay in or go out", "pick a name from Ana, Luis and
Marta", "help me choose: pizza or sushi". Options are separated by commas,
"o"/"or", "y"/"and" (before the last), or slashes.
**Slots** — `options` (term list, required, at least one).
**Answers** — the chosen option exactly as the user wrote it (trailing
punctuation removed). Each distinct option is equally likely; repeated options
(ignoring case) count once. One draw. If the options were separated with both
"y" and "o" (or "and" and "or"), the split may be wrong, so the reply also
lists the options it read.
**Does not** —
- "pizza o sushi?" with no verb of choosing: not a pick; it asks for Talos's
  preference, the reflector's S16 (`deflected`, [reflector.md](reflector.md)).
- "elige un número entre 1 y 10": `random.number`.
- "tira una moneda": `random.coin`.
- "¿cuál es mejor, pizza o sushi?": an opinion question; the reflector's S16.
- "qué cenamos": no options given; `unhandled`.

#### Examples

```text
Given: draw = option 2
[es] U: elige entre pizza, sushi y tacos
     T: Elijo sushi.                                  → answered · random.pick · options=pizza|sushi|tacos

Given: draw = option 3
[es] U: escoge uno: rojo, verde, azul
     T: Azul.                                         → answered · random.pick · options=rojo|verde|azul

Given: draw = option 1
[es] U: decide por mi: ir al cine o quedarnos en casa
     T: Ir al cine.                                   → answered · random.pick · options=ir al cine|quedarnos en casa

Given: draw = option 3
[es] U: sortéame a ana, luis y marta
     T: Marta.                                        → answered · random.pick · options=ana|luis|marta

Given: draw = option 2
[es] U: no se que cenar, elige entre pasta, pizza o ensalada
     T: Pizza.                                        → answered · random.pick · options=pasta|pizza|ensalada

Given: draw = option 1
[es] U: elige: té o café
     T: Té.                                           → answered · random.pick · options=té|café

Given: draw = option 3
[en] U: pick one: red, green, blue
     T: Blue.                                         → answered · random.pick · options=red|green|blue

Given: draw = option 1
[en] U: choose between tea and coffee
     T: Tea.                                          → answered · random.pick · options=tea|coffee

Given: draw = option 2
[en] U: decide for me: stay in or go out
     T: Go out.                                       → answered · random.pick · options=stay in|go out

Given: draw = option 3
[en] U: pick a name from Ana, Luis and Marta
     T: Marta.                                        → answered · random.pick · options=Ana|Luis|Marta

Given: draw = option 2
[en] U: pick between pizza, sushi, tacos
     T: Sushi.                                        → answered · random.pick · options=pizza|sushi|tacos

Given: draw = option 1
[en] U: help me choose: pizza or sushi
     T: Pizza.                                        → answered · random.pick · options=pizza|sushi
```

#### Edge cases

```text
[es] U: elige uno
     T: ¿Entre qué opciones?                          → asks · options
[es] U: pizza, sushi o tacos
     T: Elijo tacos.                                  → answered · random.pick · options=pizza|sushi|tacos
     (given draw = option 3; the open question takes the reply)

[en] U: pick one
     T: Pick among what?                              → asks · options

[es] U: ayúdame a elegir entre dos camisetas
     T: ¿Cuáles son las dos? Dime sus nombres.        → asks · options
     ("two shirts" names no options)

[es] U: elige entre pizza
     T: Solo me das una opción: pizza.                → answered · random.pick · options=pizza
     (one option: it is returned, and the reply says there was no choice)

[en] U: choose between tea or tea or coffee
     T: Coffee.                                       → answered · random.pick · options=tea|coffee
     (given draw = option 2; the repeated "tea" counts once)

[en] U: choose between tea or tea
     T: There's only one option: tea.                 → answered · random.pick · options=tea

[es] U: elige entre pan y queso o jamón
     T: Elijo jamón (entre pan, queso y jamón).       → answered · random.pick · options=pan|queso|jamón
     (given draw = option 3; "y" and "o" both appear, so the reading of the options is shown)

[es] U: elige entre 1 y 3
     T: Elijo 3.                                      → answered · random.pick · options=1|3
     (two numbers with no word "número": the two numbers are the two options, 2 is not eligible)

[es] U: elige un número entre 1 y 3
     T: 2.                                            → answered · random.number
     (the word "número" makes it a range)

[en] U: pizza or sushi?
     T: I don't have likes or dislikes. Do you?       → deflected · ask-opinion
     (no verb of choosing; not a pick: the reflector's S16)

[es] U: qué es mejor, pizza o sushi
     T: No tengo gustos de ningún tipo. ¿Y tú?        → deflected · ask-opinion
     (an opinion question is not a pick)

[en] U: pick a color
     T: Pick among what?                              → asks · options
     (no list; Talos does not invent the options)
```

### random.number

**Recognises** — a random number in a range: "dame un número del 1 al 10",
"número aleatorio entre 1 y 100", "un número al azar", "dime un número entre -5
y 5", "5 números del 1 al 49 sin repetir", "del uno al seis"; "give me a random
number between 1 and 10", "random number", "pick a number from 1 to 6", "a
number between -5 and 5", "5 numbers from 1 to 49 without repeating", "a random
number up to 50".
**Slots** — `low` (number, default 1), `high` (number, default 100), `count`
(number, default 1, 1 to 100), `distinct` (default no).
**Answers** — the number. Both bounds are inclusive. If both bounds are whole
numbers the result is a whole number; if either has decimals the result has two
decimals. "Hasta 50"/"up to 50" means 1 to 50. With a count, the numbers are
listed in the order drawn, with repetition allowed unless the user says "sin
repetir"/"without repeating"/"distinct". One draw per number.
**Does not** —
- "tira un dado", "d20": `random.dice`.
- "elige entre 1, 2 y 3": `random.pick`.
- "adivina el número": a game, not specified yet.
- "un número de teléfono", "a random name": not a number in a range;
  `unhandled`.

#### Examples

```text
Given: draw = 7
[es] U: dame un número del 1 al 10
     T: 7.                                            → answered · random.number · low=1 high=10

Given: draw = 42
[es] U: número aleatorio entre 1 y 100
     T: 42.                                           → answered · random.number · low=1 high=100

Given: draw = 58
[es] U: un numero al azar
     T: 58.                                           → answered · random.number · low=1 high=100

Given: draw = -2
[es] U: dime un número entre -5 y 5
     T: -2.                                           → answered · random.number · low=-5 high=5

Given: draw = 3, 17, 22, 41, 8
[es] U: 5 números del 1 al 49 sin repetir
     T: 3, 17, 22, 41, 8.                             → answered · random.number · low=1 high=49 count=5 distinct

Given: draw = 1.73
[es] U: numero entre 0,5 y 2,5
     T: 1,73.                                         → answered · random.number · low=0.5 high=2.5

Given: draw = 4
[es] U: del uno al seis
     T: 4.                                            → answered · random.number · low=1 high=6

Given: draw = 7
[en] U: give me a random number between 1 and 10
     T: 7.                                            → answered · random.number · low=1 high=10

Given: draw = 58
[en] U: random number
     T: 58.                                           → answered · random.number · low=1 high=100

Given: draw = 4
[en] U: pick a number from 1 to 6
     T: 4.                                            → answered · random.number · low=1 high=6

Given: draw = -2
[en] U: a number between -5 and 5
     T: -2.                                           → answered · random.number · low=-5 high=5

Given: draw = 3, 17, 22, 41, 8
[en] U: 5 numbers from 1 to 49 without repeating
     T: 3, 17, 22, 41, 8.                             → answered · random.number · low=1 high=49 count=5 distinct

Given: draw = 1.73
[en] U: random number between 0.5 and 2.5
     T: 1.73.                                         → answered · random.number · low=0.5 high=2.5

Given: draw = 31
[en] U: a random number up to 50
     T: 31.                                           → answered · random.number · low=1 high=50
```

#### Edge cases

```text
Given: draw = 4
[es] U: un número del 10 al 1
     T: 4 (entre 1 y 10).                             → answered · random.number · low=1 high=10
     (reversed bounds are put in order, and the reply shows the range used)

[en] U: random number between 5 and 5
     T: 5. The range had only one value.              → answered · random.number · low=5 high=5

[es] U: un número entre 1 y 10^30
     T: El rango es demasiado grande; llego hasta 10^15. → unhandled · random.number

[en] U: 10 numbers from 1 to 5 without repeating
     T: I can't draw 10 different numbers from only 5.
                                                      → answered · random.number
     (impossible, so it is explained, not attempted)

Given: draw = 2, 2, 5
[es] U: 3 numeros del 1 al 5
     T: 2, 2, 5.                                      → answered · random.number · count=3
     (repetition is allowed unless the user says otherwise)

[en] U: random number between 1 and
     T: Between 1 and what?                           → asks · high

[es] U: 150 numeros del 1 al 10
     T: Como mucho doy 100 números a la vez.          → unhandled · random.number

[en] U: give me a random phone number
     T: I only give numbers in a range.               → unhandled

[es] U: adivina el número que pienso
     T: (games are not specified yet)                 → unhandled
```

---

## fortune

A saying drawn from a bundled list, in the conversation language. Nothing is
composed: every reply is an entry of the list, with its attribution if it has
one. The lists are written per language (Spanish refranes and sayings, English
proverbs and fortunes), not translated from one another. The skill does not
predict anything and does not claim to; a "fortune" is a fortune-cookie line,
not a forecast.

### fortune.tell

**Recognises** — asking for a saying: "dime un refrán", "una frase", "dime un
dicho", "un proverbio", "galleta de la fortuna", "dime algo sabio", "dime la
suerte", "fortune"; "tell me a fortune", "give me a proverb", "fortune cookie",
"say something wise", "give me a saying", "tell me my fortune", "fortune".
**Slots** — none.
**Answers** — one entry of the bundled list for the conversation language. One
draw. Consecutive fortunes in one conversation are never the same entry (while
the list has more than one). Right after a fortune, "otro"/"another one" is a
fortune again; anywhere else it is not.
**Does not** —
- "cuéntame un chiste", "tell me a joke": jokes are not covered; `unhandled`.
- "horóscopo de escorpio", "what will happen to me": predictions are not made;
  `unhandled`, said plainly.
- "dime un refrán sobre perros", "a proverb about money": the list is not
  searchable by topic; `unhandled`, said plainly.
- "qué significa «a caballo regalado…»", "what does that proverb mean":
  explaining a saying is not covered; `unhandled`.
- "escríbeme un poema", "write me a quote": generation; `unhandled`.
- "una cita de Einstein": looking quotes up by author is not covered; `unhandled`.
- "¿me saldrá bien?", "dime sí o no": a fortune never decides or answers a
  question. With options and a verb of choosing ("elige por mí: sí o no") it
  is `random.pick`; "¿debería…?" is advice, the reflector's S27.

#### Examples

The entries shown are illustrative; the bundled lists decide which sayings
exist.

```text
Given: draw = entry "a caballo regalado"
[es] U: dime un refrán
     T: A caballo regalado no le mires el diente.     → answered · fortune.tell

Given: draw = entry "quien mucho abarca"
[es] U: galleta de la fortuna
     T: Quien mucho abarca, poco aprieta.             → answered · fortune.tell

Given: draw = entry "no hay mal"
[es] U: un proverbio
     T: No hay mal que por bien no venga.             → answered · fortune.tell

Given: draw = entry "mas vale tarde"
[es] U: dime algo sabio
     T: Más vale tarde que nunca.                     → answered · fortune.tell

Given: draw = entry "de tal palo"
[es] U: suéltame un dicho anda
     T: De tal palo, tal astilla.                     → answered · fortune.tell

Given: draw = entry "camaron"
[es] U: dime mi suerte
     T: Camarón que se duerme se lo lleva la corriente. → answered · fortune.tell

Given: draw = entry "stitch in time"
[en] U: tell me a fortune
     T: A stitch in time saves nine.                  → answered · fortune.tell

Given: draw = entry "early bird"
[en] U: give me a proverb
     T: The early bird catches the worm.              → answered · fortune.tell

Given: draw = entry "thousand miles"
[en] U: fortune cookie
     T: A journey of a thousand miles begins with a single step. → answered · fortune.tell

Given: draw = entry "fortune favours"
[en] U: say something wise
     T: Fortune favors the bold.                      → answered · fortune.tell

Given: draw = entry "rome"
[en] U: give me a saying
     T: Rome wasn't built in a day.                   → answered · fortune.tell

Given: draw = entry "chickens"
[en] U: fortune
     T: Don't count your chickens before they hatch.  → answered · fortune.tell
```

#### Edge cases

```text
Given: draw = entry "a caballo regalado" (the entry shown on the previous fortune turn)
[es] U: dime un refrán
     T: (a different entry: the same one never twice in a row) → answered · fortune.tell
     (the source is redrawn until the entry differs from the last one in this conversation)

[es] U: dime un refrán
     T: Más vale tarde que nunca.                     → answered · fortune.tell
[es] U: otro
     T: De tal palo, tal astilla.                     → answered · fortune.tell
     (right after a fortune, "otro" means another fortune)

[en] U: tell me a fortune
     T: Rome wasn't built in a day.                   → answered · fortune.tell
[en] U: another one
     T: A stitch in time saves nine.                  → answered · fortune.tell

[en] U: another one
     T: I didn't understand that.                     → unhandled
     (no earlier fortune: nothing to repeat; no reflector shape takes it, so
      it is the catch-all, reflector.md 4.7)

[es] U: cuéntame un chiste
     T: No cuento chistes.                            → unhandled

[en] U: what will happen to me this year?
     T: I can't predict anything.                     → unhandled
     (honest about not predicting; no offer of a saying instead, which would
      be an unsolicited suggestion)

[es] U: horóscopo de escorpio
     T: No hago horóscopos.                           → unhandled

[es] U: dime un refrán sobre perros
     T: No puedo buscar refranes por tema.            → unhandled

[en] U: what does that proverb mean?
     T: I can't explain sayings.                      → unhandled

[en] U: write me an inspirational quote
     T: I can't write texts; I can only give a saying from my list.
                                                      → unhandled
     (generation is always unhandled)

[es] U: una cita de Einstein
     T: No busco citas por autor.                     → unhandled
```

---

## Assumptions in this file

Choices made where the design documents were silent. Each is a candidate for a
decision in review.

- Status of impossible and unsupported requests: an impossible but well-formed
  request (÷0, incompatible units, a non-existent date) is `answered`; a
  request outside what is supported (unknown unit, parse limit, too large) is
  `unhandled` with the intent kept on the outcome line. This is the rule
  later written into [README.md](README.md), "Which status an outcome gets".
- `clock.date` also answers the date of a relative day ("mañana", "el próximo
  viernes", "dentro de 100 días"), and `clock.now` accepts an offset ("dentro de
  3 horas"). The catalogue describes both as "today's date" and "the current
  time" only.
- "next Friday" is the first Friday strictly after today; the resolved date is
  always stated so the user can correct it.
- Spanish uses Spain's conventions (24-hour clock, day before month). English
  uses the 24-hour clock and day before month, as conversation.md, state.md
  and knowledge.md do, and asks about numeric dates (cross-review
  resolution; see [decisions.md](decisions.md)). A user preference for other
  conventions is not specified here; it is a later addition. **Ruled (ruling
  7).**
- The "otro"/"another one" follow-up after a fortune relies on the recent-topic
  dialog state; its details belong to [conversation.md](conversation.md).
- Who created Talos is answered (`answered · identity.who`) with a fixed,
  configurable identity line, by default "a project by Pixelotes"; no
  individual and no other company is named. The origin of the name and its age
  remain `unhandled · identity.who`. **Ruled (ruling 4).**
- Randomness is described only by what a test can observe (seed, draws per
  intent, determinism); the generator itself is not specified. It is derived
  from the conversation, turn number and text, never from system entropy.
