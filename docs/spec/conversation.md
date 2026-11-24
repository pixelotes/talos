# Conversation: language, slots, open questions, errors

Cross-cutting behaviour. No skill owns it; every skill inherits it. Format and
global rules are in [README.md](README.md). Pipeline stages referred to here
are in [DESIGN.md](../DESIGN.md) section 4.

Slot names in the outcome lines of this file are illustrative (the owning
skill's section in the other spec files is normative). What is normative here
is how a phrasing is *understood*.

Unless stated otherwise: `Given: now = 2027-03-14 18:42 Europe/Madrid`, which
is a Sunday.

---

## 1. Language

### 1.1 Which language a turn is in

- **A caller's hint wins.** If the turn carries a language hint, that is the
  language of the turn and of the reply. No detection runs, even if the text
  looks like the other language. (A host that knows better than Talos is
  right by definition.)
- **Without a hint, Talos detects** between Spanish and English only. The
  evidence is function words ("el", "de", "qué", "the", "is", "what") and
  character patterns. Accents are evidence but their absence is not: text
  typed without accents ("que hora es en tokio") is still Spanish.
- **The result is sticky per conversation.** Once a conversation is in a
  language, a turn stays in it unless the evidence for the other language is
  clear. One ambiguous word never switches anything.
- **The first turn of a conversation with no evidence at all** (a single
  ambiguous word) is answered in the configured default language.
- **A reply is always in the language of the turn it answers**, including
  refusals and errors.

### 1.2 Words that do not decide

Single words and short strings that are valid in both languages carry no
language evidence: "ok", "vale" (Spanish, but one word is not enough to switch
away from English), "no", "chess", "pizza", "wifi", "hola" after English
turns, "thanks" after Spanish turns, names, numbers, emoji, and bare place
names. Such a turn is answered in the conversation's current language, and
the conversation's language does not change.

```text
Given: a conversation that has been in Spanish.

[es] U: ok
     T: Vale.                                  → answered · no intent · language stays es

[es] U: chess
     T: Eso todavía no lo sé hacer.            → unhandled · language stays es
```

```text
Given: a new conversation, no hint, default language en.

[en] U: ok
     T: Okay.                                  → answered · no intent
```

An acknowledgement ("ok", "vale", "okay", "genial") with nothing to
acknowledge gets a minimal acknowledgement back and nothing else. It never
asks a question, because a follow-up question would be an unsolicited
suggestion.

### 1.3 A genuine switch

A switch happens when a turn contains clear evidence for the other language:
a whole phrase with several function words, or a recognised intent phrasing in
that language. The switch applies **from that turn on**: that turn's reply is
already in the new language, and the next turns stay in it, again with
stickiness.

```text
Given: a conversation that has been in Spanish.

[es] U: qué hora es en Tokio
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio

[en] U: and what's the time in London?
     T: In London it's 17:42.                  → answered · timezones.time_in · place=London
     (clear English: the conversation is now en)

[en] U: ok
     T: Okay.                                  → answered · no intent · language stays en

[en] U: gracias
     T: You're welcome.                        → answered · smalltalk.thanks
     (one word is not enough to switch back; the reply stays in English)
```

A switch back requires clear evidence again ("y en Berlín, por favor" is
clear; "gracias" is not).

The "gracias" case answers in English, not in Spanish. That is deliberate: a
user who types one stray word of another language has not asked to change
language. If they keep writing in it, the next full phrase switches.

### 1.4 Mixed-language input

Real users write "pon un timer", "dime el weather", "qué hora es in Tokyo".
Rules:

- The **language of the turn is the language of its grammar**, i.e. of the
  function words and the verb, not of the loanwords. "pon un timer" is
  Spanish; "what's the hora in Tokyo" is English.
- **Loanwords and foreign names are accepted as content**: place names in
  either language ("Nueva York", "New York"), terms, and technical words
  ("timer", "email", "weekend", "finde") fill slots and match intents as long
  as the intent exists.
- A mixed turn **does not switch the conversation** unless its grammar is in
  the other language.
- A request for something Talos does not do stays unhandled whatever
  language it is in.

```text
Given: a conversation in Spanish.

[es] U: qué hora es in Tokyo
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokyo

[es] U: pon un timer de 5 minutos
     T: Eso no lo puedo hacer: no sé avisarte pasado un tiempo.
                                               → unhandled
     (the language is Spanish; timers are simply not a Talos skill)

[es] U: dime el finde que viene qué día cae
     T: El finde no es un día concreto; no sé qué fecha tomar.
                                               → unhandled · clock.weekday
     (the intent is recognised; "finde" is not a single date, 2.3)
```

```text
Given: a conversation in English.

[en] U: what's the hora in Tokyo
     T: In Tokyo it's 02:42 (already Monday).  → answered · timezones.time_in · place=Tokyo
     (English grammar, one Spanish word: stays en; "hora" is read as "time")

[en] U: set a timer for five minutos
     T: I can't do that: I can't alert you after a delay.
                                               → unhandled
```

If a loanword makes an intent ambiguous, the ordinary rules apply (the
classifier's threshold and margin); a mixed turn is not penalised for being
mixed, only for being unclear.

---

## 2. Slots

Slot types are shared and typed: `number`, `duration`, `datetime`, `place`,
`language`, `term`, `choice`. Each is understood in both languages, whatever
the skill that asks for it. The tables below list phrasings that **must** be
understood; a type may understand more.

Matching ignores case, accents, doubled spaces and trailing punctuation:
"MAÑANA", "manana", "mañana?" are the same.

### 2.1 `number`

Understood in digits, in words, and as a mix.

**Spanish.** Words from "cero" to the millions: "tres", "veintiuno",
"veintiún", "treinta y cinco", "cien", "ciento diez", "mil quinientos", "dos
mil veintisiete", "un millón", "dos millones y medio". Fractions and
quantities: "medio", "media", "un cuarto", "tres cuartos", "una docena",
"la mitad", "el doble" (only where a skill gives them a reference). Negative:
"menos tres", "-3". Words and digits mixed: "3 mil", "2 millones".

**English.** "zero" to the millions: "three", "twenty-one", "twenty one",
"a hundred", "one hundred and ten", "fifteen hundred", "two thousand
twenty-seven", "a million", "two and a half million". Fractions and
quantities: "half", "a quarter", "three quarters", "a dozen". Negative:
"minus three", "negative three", "-3". Mixed: "3 thousand", "2 million".

**Decimal and thousands separators depend on the language of the turn**:

| Written | In a Spanish turn | In an English turn |
|---|---|---|
| `1.500` | 1500 (a dot followed by exactly three digits is a thousands separator) | 1.5 (a dot is always a decimal point) |
| `1,500` | 1.5 (a comma is the decimal separator) | 1500 (a comma followed by exactly three digits is a thousands separator) |
| `1.5` | 1.5 | 1.5 |
| `1,5` | 1.5 | 1.5 (lenient: a comma not followed by three digits is read as a decimal) |
| `1 500` | 1500 | 1500 |
| `1.500.000` | 1500000 | not a number; ask |
| `1,500,000` | not a number; ask | 1500000 |
| `12.34.56` | not a number | not a number |

Two things follow. First, the same text can mean different numbers in the two
languages; this is intended, because it is what a native writer means. Second,
when separators are mixed or malformed (`1.500,25` in Spanish is 1500.25 and
`1,500.25` in English is 1500.25; the opposite conventions are not accepted),
Talos does not guess: a malformed number is a missing slot and is asked back
(see section 3).

Digit grouping is never inferred from the *other* language's convention to
rescue a turn.

Examples of understanding (illustrative outcome slots):

```text
[es] U: cuánto es mil quinientos más 250
     T: 1750.                                  → answered · calc.arithmetic · numbers=1500,250
     (Spanish replies group thousands with a dot from five digits up, so 1750
      has none; see basics.md, "Numbers in and out")

[es] U: cuánto es 1.500 más 250
     T: 1750.                                  → answered · calc.arithmetic · numbers=1500,250

[es] U: cuánto es 1,5 por 4
     T: 6.                                     → answered · calc.arithmetic · numbers=1.5,4

[en] U: what is 1,500 plus 250
     T: 1,750.                                 → answered · calc.arithmetic · numbers=1500,250
     (English replies group thousands with a comma)

[en] U: what is 1.5 times 4
     T: 6.                                     → answered · calc.arithmetic · numbers=1.5,4

[en] U: what's two and a half million divided by five
     T: 500,000.                               → answered · calc.arithmetic · numbers=2500000,5
```

Replies format numbers by the language of the reply, not of the number's
source. Talos never rounds a number silently: if it rounds, the reply says so.

### 2.2 `duration`

An amount of time, with or without a start.

**Spanish.** "5 minutos", "cinco minutos", "una hora", "hora y media", "media
hora", "un cuarto de hora", "tres cuartos de hora", "una hora y cuarto", "dos
horas y media", "90 minutos", "1h30", "1:30 h", "45 min", "30 seg", "tres
días", "una semana", "quince días" (two weeks), "un finde" is *not* a
duration. "Un rato", "un ratito", "un momento", "unas horas", "un par de
horas" (vague) are not durations except "un par de" with a unit, which means
two.

**English.** "5 minutes", "five minutes", "an hour", "an hour and a half",
"half an hour", "a quarter of an hour", "three quarters of an hour", "90
minutes", "1h30", "1h 30m", "45 min", "30 secs", "three days", "a week", "a
fortnight", "a couple of hours" (two). "A while", "a bit", "some time" are
not durations.

Rules:

- A duration with a vague quantity is a **missing slot**, not a guess: Talos
  asks for a number.
- "Quince días" means 15 days in Spanish and is read as such; Talos does not
  silently convert it to two weeks in its reply.
- Month and year lengths are calendar-based, not 30 or 365 days.
- A duration may be negative only where the skill talks about "ago"/"hace".

```text
[es] U: cuánto son media hora y 20 minutos
     T: 50 minutos.                            → answered · calc.arithmetic · durations=30min,20min

[en] U: how much is an hour and a half plus 45 minutes
     T: 2 hours 15 minutes.                    → answered · calc.arithmetic · durations=90min,45min
```

### 2.3 `datetime`

A date, a time of day, or both. Relative to `now`; the time zone is the
conversation's (see the skill, and [state.md](state.md) for how a remembered
place sets it).

**Times of day.**

- Spanish: "a las 15:30", "las 3 y media", "tres y media", "las tres y
  cuarto", "las cuatro menos cuarto", "las cuatro menos diez", "las 5 de la
  tarde", "las 9 de la noche", "a las 8 de la mañana", "mediodía",
  "medianoche", "las doce de la noche", "las 17h", "17.30". The 24-hour clock
  is the default in writing; "y media" means :30, "y cuarto" :15, "menos
  cuarto" :45, "menos veinte" :40.
- English: "3:30 pm", "3.30pm", "15:30", "half past three", "quarter past
  three", "quarter to four", "ten to four", "three thirty", "5 in the
  afternoon", "9 at night", "noon", "midnight", "a.m.", "p.m.".
- **A bare hour without am/pm or a day part** ("las 5", "at five", "tres y
  media") is read as the **next time that clock reading occurs after `now`**.
  At 18:42, "las 5" is 05:00 tomorrow, "las 7" is 19:00 today. A day-part word
  overrides: "las 5 de la tarde" is 17:00.

**Days and dates.**

- Spanish: "hoy", "mañana", "pasado mañana", "ayer", "anteayer", "el lunes", "el
  lunes que viene", "este viernes", "el viernes pasado", "dentro de tres
  días", "hace dos semanas", "el 3 de marzo", "el 3/4", "3-4-2027", "el día
  3", "a finales de mes", "el uno de enero".
- English: "today", "tomorrow", "the day after tomorrow", "yesterday", "on
  Monday", "next Monday", "this Friday", "last Friday", "in three days", "two
  weeks ago", "March 3rd", "3 March", "the 3rd", "3/4", "end of the month".
- A weekday name alone ("el lunes", "on Monday") means the **next such day
  after today**. If that name is today's weekday, it means **seven days from
  today** unless the user said "hoy"/"today" or "este"/"this". ("This Sunday"
  said on a Sunday is today.)
- **Numeric dates follow the language of the turn**: in Spanish `3/4` is 3
  April (day first). In English `3/4` is **ambiguous** (3 April in much of the
  world, 4 March in the US) and Talos asks (section 3.4). A numeric date that
  only one reading makes valid (`25/12`; in English also `12/25`) is not
  ambiguous. Replies write dates out in words, day before month in both
  languages ("3 April", "el 3 de abril").
- Two-digit years are 20xx. A date without a year is the **next occurrence**
  on or after today.
- "Fin de semana"/"weekend" is not a point in time; it is not accepted where
  a single date is required.

**Day parts without an hour.** A day part by itself is a *range*, and skills
that need one moment read it at a fixed representative time:

| Part | Spanish | English | Range | Representative |
|---|---|---|---|---|
| morning | "por la mañana", "de mañana" | "in the morning" | 06:00-12:00 | 09:00 |
| midday | "a mediodía" | "at noon" | | 12:00 |
| afternoon | "por la tarde" | "in the afternoon" | 12:00-20:00 | 17:00 |
| evening/night | "por la noche", "esta noche" | "in the evening", "tonight", "at night" | 20:00-24:00 | 21:00 |
| small hours | "de madrugada" | "in the small hours" | 00:00-06:00 | 03:00 |

In Spanish "tarde" begins after lunch (about 14:00) and "noche" after dark, in
line with Spain's meal times; English "evening" starts earlier (about 18:00).
The ranges above are the ones Talos uses in both languages; the
representative times are used only when a skill needs a single moment.

"**Mañana por la mañana**" is tomorrow morning: the first "mañana" is the day
because it is not preceded by "por la"/"de la"; the second is the day part. "Mañana por la tarde" is tomorrow afternoon; "mañana por la noche" is tomorrow
night; "esta tarde" is today's afternoon. English: "tomorrow morning",
"tomorrow night", "this afternoon", "tonight".

```text
Given: now = 2027-03-14 18:42 Europe/Madrid (Sunday).

[es] U: cuánto falta para mañana por la tarde
     T: Faltan 22 horas y 18 minutos (mañana a las 17:00).
                                               → answered · clock.until · datetime=2027-03-15 17:00
[es] U: cuánto falta para las 3 y media
     T: Faltan 8 horas y 48 minutos (mañana a las 03:30).
                                               → answered · clock.until · datetime=2027-03-15 03:30
[es] U: cuánto falta para las cuatro menos cuarto de la tarde
     T: Faltan 21 horas y 3 minutos (mañana a las 15:45).
                                               → answered · clock.until · datetime=2027-03-15 15:45
[es] U: cuánto falta para el 3/4
     T: Faltan 20 días (el 3 de abril).        → answered · clock.until · datetime=2027-04-03

[en] U: how long until half past three
     T: 8 hours 48 minutes (tomorrow at 03:30).
                                               → answered · clock.until · datetime=2027-03-15 03:30
[en] U: how long until tomorrow afternoon
     T: 22 hours 18 minutes (tomorrow at 17:00).
                                               → answered · clock.until · datetime=2027-03-15 17:00
[en] U: how long until next Friday
     T: 5 days (Friday 19 March).              → answered · clock.until · datetime=2027-03-19
     (the reply states the date it took)
[en] U: how long until 3/4
     T: Do you mean 3 April or 4 March?        → asks · datetime
```

Whenever Talos has chosen an interpretation of a vague or relative datetime,
its reply **states the concrete date or time it used**, so a wrong reading is
visible and correctable.

### 2.4 `place`

A city, a country, a region, an alias, or a reference to a remembered place.

- **Names in both languages are the same place**: "Nueva York"/"New York",
  "Londres"/"London", "Múnich"/"Munich", "Pekín"/"Beijing", "Estados
  Unidos"/"United States"/"EEUU"/"USA"/"US", "Ciudad de México"/"Mexico
  City"/"CDMX", "Sevilla"/"Seville", "San Petersburgo"/"Saint Petersburg",
  "Moscú"/"Moscow", "Nápoles"/"Naples".
- **Common short forms and informal names**: "NYC", "LA", "Madrid capital",
  "la Gran Manzana" is *not* recognised (Talos does not understand
  nicknames).
- **Spelling variation** is tolerated for names (accents, one-letter typos on
  long names: "Sidney" for "Sydney", "Estambul"/"Istanbul"/"Istambul").
- **Time-zone names and offsets** are places for the skills that care about
  time: "UTC", "GMT", "UTC+2", "GMT-5", "CET", "hora peninsular", "EST".
- **Relative references**: "mi ciudad", "mi casa", "mi sitio", "home",
  "where I live" refer to the **remembered place** of the user
  ([state.md](state.md)). "aquí", "acá", "aqui", "here", "where I am" refer
  to where the user is now: the remembered place, else the caller's place;
  a question that needs only a time zone ("qué hora es aquí") is `clock.now`
  and falls back to the server's zone (state.md, precedence). "allí", "allá",
  "there" refer to the place named last in this conversation. If no such
  place exists, the slot is missing and is asked back.
- **A country** stands for its capital's behaviour only where one capital's
  time or answer represents the country. A country that spans zones with no
  single representative (the United States, Canada, Russia, Brazil,
  Australia) is a missing slot: Talos asks which city.
- **Ambiguous names** ("Córdoba": Spain/Argentina; "Valencia": Spain/Venezuela;
  "Santiago"; "Birmingham"; "San José"; "Portland"): if the candidates give
  different answers for what was asked, Talos asks which one, as a `choice`.
  If they give the same answer, Talos answers and names the place it took.
- **Unknown names** are a failure, not a guess: Talos says it does not know
  that place (status `unhandled`, intent kept).

### 2.5 `language`

Names a language: "inglés", "español", "castellano", "en inglés", "al
español", "English", "Spanish", "in Spanish", "into English". Both languages'
names for languages are accepted in both conversation languages ("el
English" is not; "in español" is).

Other languages are *recognised as languages* ("francés", "French", "alemán",
"German", "catalán", "Catalan") and then reported as unsupported by the
skill that asked; they are never silently treated as Spanish or English.

### 2.6 `term`

A word or phrase taken as content, not interpreted. It is, in order of
preference:

1. The text between quotes of any common kind: `"..."`, `'...'`, curly
   quotes, `«...»`.
2. The trailing phrase after a cue ("define *serendipia*", "qué significa
   *tener morriña*", "what does *to be on the fence* mean").
3. For a single-word slot, the one word left after the intent's words are
   removed.

A term keeps its original spelling, case and accents; it is not normalised
before the skill sees it. Surrounding politeness words ("por favor", "please",
"gracias") are not part of a term. A term inside quotes is **never**
interpreted as a command, a number or a place.

### 2.7 `choice`

One option out of a short list Talos has just offered or the skill declares.

- By its **name**, in either language where the option has a name in both
  ("metric"/"métrico").
- By **position**: "el primero", "la segunda", "el último", "the first one",
  "the second", "the last one", "1", "2", "la 2", "number two", "option b" when
  options were labelled.
- By **partial but unique** text ("la argentina", "the Spanish one").
- For a two-way question, "sí"/"yes" selects the offered option when it was
  phrased as a proposal ("¿Te refieres a Córdoba, en España?"), and "no"
  selects nothing: Talos then asks again or offers the other option; it
  never guesses.
- "Ninguna", "none", "ni idea" / "no idea" decline all options and close the
  question.

A choice answer that matches nothing is an unrelated message (section 3.5);
it does not make Talos pick one.

---

## 3. Open questions (expectations)

### 3.1 When Talos asks back

When a **required** slot is missing, or a number, date or place is
malformed or ambiguous, the skill asks one short question naming what is
missing. It opens an *expectation*, typed by the slot it needs (a place, a
number, a choice, yes/no). Talos asks **at most one question at a time**, and
never asks for an optional slot.

The question is concrete and short, and a user can answer with only the slot
value ("Tokio", "Tokyo"), a whole sentence, or a corrected request.

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place   (timezones.time_in)

[en] U: what time is it in
     T: In which city?                         → asks · place   (timezones.time_in)

[es] U: ¿cuántas horas de diferencia hay con?
     T: ¿Con qué lugar?                        → asks · place   (timezones.difference)
```

### 3.2 The next message answers it

The expectation sees the **next message first**, before rules and classifier.
It accepts it if it contains a value of the expected type: bare ("Tokio"),
framed ("en Tokio", "in Tokyo please", "pues Tokio", "Tokio, anda") or
inside a repeated request ("la hora en Tokio"). The original request is then
completed with the value and answered; the outcome is the *original* intent.

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place
[es] U: Tokio
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio

[en] U: what's the time in
     T: In which city?                         → asks · place
[en] U: tokyo please
     T: In Tokyo it's 02:42 (already Monday).  → answered · timezones.time_in · place=Tokyo
```

- The answer may be in **either language**. The expectation does not belong to
  a language; the reply is in the language of the answering turn.

  ```text
  [es] U: qué hora es en
       T: ¿En qué ciudad?                      → asks · place
  [en] U: London, thanks
       T: In London it's 17:42.                → answered · timezones.time_in · place=London
  ```

- A **correction** inside the answer replaces the value: "no, en Osaka", "perdona,
  Osaka", "sorry, I meant Osaka".
- An answer that carries a value *and* a new intent ("Tokio. Y dime la
  fecha") answers the open question first and then, in the same reply, the
  new request if it is a clearly separate sentence; otherwise only the open
  question is answered and the rest is ignored. Talos never answers two
  intents when it is unsure where the first ends.
- An answer that is **the right type but invalid** (an unknown place, a
  malformed number) is not accepted as an answer; Talos says it did not
  understand the value and keeps the question open for the next turn (this
  counts toward expiry).

### 3.3 Yes/no and choices

An expectation of the yes/no kind accepts "sí", "si", "vale", "claro", "por
supuesto", "venga", "adelante", "ok", "dale" / "yes", "yeah", "yep", "sure",
"ok", "go ahead" as yes, and "no", "nope", "mejor no", "no, gracias", "ni
hablar" / "no", "nah", "don't", "no thanks" as no. "Vale" is yes only when a
yes/no is open.

```text
Given: stored place = Sevilla.

[es] U: olvida todo lo que sabes de mí
     T: ¿Seguro? Borraré tu nombre, tu lugar y tus unidades.
                                               → asks · choice (yes/no)   (preferences.forget)
[es] U: sí
     T: Hecho. Ya no recuerdo nada sobre ti.   → answered · preferences.forget · key=all

[en] U: forget everything about me
     T: Are you sure? I'll delete your name, location and units.
                                               → asks · choice (yes/no)   (preferences.forget)
[en] U: no
     T: Okay, nothing deleted.                 → answered · preferences.forget · nothing deleted
```

A "no" is an answer, like a "yes": the skill says nothing was done and the
outcome keeps the original intent. A cancellation (3.5) is not an answer and
has no intent.

### 3.4 Asking again

If the answer is the right type but ambiguous (a place with several
candidates, a numeric date), Talos asks a narrower question, as a `choice`,
instead of the generic one:

```text
[es] U: qué hora es en Córdoba
     T: ¿Córdoba de España o de Argentina?     → asks · choice   (timezones.time_in)
[es] U: la argentina
     T: En Córdoba (Argentina) son las 14:42.  → answered · timezones.time_in · place=Córdoba (AR)

[en] U: what time is it in Cordoba
     T: Cordoba in Spain or in Argentina?      → asks · choice   (timezones.time_in)
[en] U: the first one
     T: In Córdoba (Spain) it's 18:42.         → answered · timezones.time_in · place=Córdoba (ES)
```

Talos never asks the same question a third time. After two failed answers
it drops the expectation and says it could not understand:

```text
[en] U: what time is it in
     T: In which city?                         → asks · place
[en] U: somewhere warm
     T: I didn't catch a place. In which city?
                                               → asks · place
[en] U: I don't know, wherever
     T: I couldn't work out a place, so I'll leave it.
                                               → unhandled · timezones.time_in
```

### 3.5 Cancelling

At any moment an open question can be cancelled in words:

- Spanish: "déjalo", "dejalo", "olvídalo", "olvidalo", "no importa", "da igual",
  "da lo mismo", "nada", "nada, nada", "cancela", "cancelar", "paso",
  "mejor no", "no te preocupes", "lo que sea".
- English: "never mind", "nevermind", "forget it", "drop it", "cancel",
  "cancel that", "leave it", "skip it", "doesn't matter", "forget about it".

Cancelling closes the expectation, answers with a minimal acknowledgement,
and does not run the original request. Words like "olvídalo" are a cancellation
**only when something is open**. With nothing open they are acknowledged just
as minimally (nothing happens), and "olvida X" / "forget X" with content is a
different request (`preferences.forget`, `teach.forget`).

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place
[es] U: déjalo
     T: Vale, lo dejo.                         → answered · no intent · expectation cancelled

[en] U: what's the time difference with
     T: With which place?                      → asks · place
[en] U: never mind
     T: Okay, dropped.                         → answered · no intent · expectation cancelled

[es] U: olvídalo
     T: Vale.                                  → answered · no intent
     (nothing was open; nothing happens)
```

### 3.6 An unrelated message

If the message contains no value for the open slot and is not a cancellation,
the expectation **releases** it. The pipeline continues from the rules stage
as if there were no question, the message is handled normally, **and the
question is closed**. Talos does not remind the user of it.

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place
[es] U: cuánto es 15 por 4
     T: 60.                                    → answered · calc.arithmetic
[es] U: Tokio
     T: Tokio... no sé qué quieres saber de Tokio.
                                               → unhandled
     (the question was closed when the unrelated message arrived)

[en] U: what's the time difference with
     T: With which place?                      → asks · place
[en] U: thanks
     T: You're welcome.                        → answered · smalltalk.thanks
     (not an answer, not a cancel: released)
```

The test for "unrelated" is: the expectation finds no acceptable value, and
the message is not a hedge (next section). A message that *is* a different
request and *also* contains a place is treated as the different request, if
its intent is recognised with certainty ("cuánto falta para el viernes en
Tokio" while a place is open): the new intent wins, because an answer is a
reply, not a command.

### 3.7 Hedges, and expiry

Some messages are neither answers nor new requests nor cancellations:
hesitation and requests for time ("mmm", "a ver", "espera", "un momento",
"déjame pensar", "hmm", "wait", "let me think", "hold on", "uh"). They
**keep the question open**, answered with a minimal acknowledgement, and they
count as turns.

An expectation expires:

- after **3 further turns** without an answer (hedges and invalid values count
  as turns), or
- when `now` has moved more than **5 minutes** past the turn that opened it,
  whichever comes first.

When it expires, the next message is handled as if there were no question.
Talos says nothing about the expiry.

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place
[es] U: mmm
     T: Tómate tu tiempo.                      → answered · no intent · expectation kept (1/3)
[es] U: a ver
     T: Vale.                                  → answered · no intent · expectation kept (2/3)
[es] U: espera
     T: Aquí sigo.                             → answered · no intent · expectation kept (3/3)
[es] U: Tokio
     T: No sé qué quieres saber de Tokio.      → unhandled
     (expired after three turns; "Tokio" alone is not a request)

Given: expectation opened at 18:42; the next message arrives at 18:49.

[en] U: Tokyo
     T: I don't know what you'd like about Tokyo.
                                               → unhandled
     (expired by time)
```

### 3.8 A bare "sí" / "no" / "yes" / "no" with nothing open

With no open question, bare confirmations have nothing to confirm.

- Talos says there is nothing pending, in the language of the conversation,
  and **does not guess** what the user might have meant.
- The status is `unhandled`; no intent. Talos does not offer help or
  suggestions.
- The word "no" is the same in both languages: the language does not change.
- "gracias"/"thanks" are not confirmations; they are `smalltalk.thanks`.

```text
[es] U: sí
     T: No tengo ninguna pregunta pendiente.   → unhandled

[es] U: no
     T: No tengo ninguna pregunta pendiente.   → unhandled

[en] U: yes
     T: There's no open question to answer.    → unhandled

[en] U: no
     T: There's no open question to answer.    → unhandled

[es] U: vale
     T: Vale.                                  → answered · no intent
     (an acknowledgement, not a confirmation; see 1.2)

[es] U: sí, por favor
     T: No tengo ninguna pregunta pendiente.   → unhandled
```

A "sí" arriving *after* an expectation has expired or been released is the
same case.

### 3.9 Distress while something is open

The distress check (reflector.md S29) runs **before** the open-question stage.
A distress message is never taken as a value, a "no", a hedge or a
cancellation, so an open yes/no or slot question cannot swallow it. The reply
is the fixed one of S29 (`unhandled · distress`), and the open question is
closed, as in 3.6. (Ruled 2026-11-24, ruling 2; an amendment to
[ADR 0004](../adr/0004-explicit-turn-pipeline.md).)

```text
[es] U: borra todo lo que sabes de mí
     T: ¿Seguro que quieres que olvide todo lo que recuerdo de ti?   → asks · confirm
[es] U: no quiero seguir viviendo
     T: Esto no puedo atenderlo yo, y es importante. Si estás en peligro, llama al 112; también tienes el 024, a cualquier hora.
                                               → unhandled · distress
     (not read as "no"; the question is closed; place known: Spain)

[en] U: forget everything about me
     T: Do you want me to forget everything I remember about you?    → asks · confirm
[en] U: I want to die
     T: I can't help with this, and it matters. If you're in danger, call your local emergency number now.
                                               → unhandled · distress
     (not read as "no"; no place known, so no number is named)
```

---

## 4. Errors and refusals

### 4.1 How Talos says it cannot

- Plainly, in one sentence, in the language of the turn, in the first
  person: "Eso no lo puedo hacer", "I can't do that."
- It says **what** it cannot do when that is clear ("No sé traducir frases
  largas", "I don't generate poems") and does not apologise at length. One
  "lo siento"/"sorry" at most, and never as filler.
- It **never pretends**: no invented answer, no "I'll try", no promise to
  do something later, no "I'm learning".
- It does not suggest alternatives, does not mention "help", and does not
  comment on the request or the user. The one exception is distress
  (reflector.md S29), which names the emergency number configured for the
  user's place.
- The status is `unhandled`. If the intent was recognised but the skill could
  not do it, the intent is kept in the outcome line (so a host can see it);
  if nothing was recognised, there is no intent.

### 4.2 Requests Talos does not understand

Gibberish, fragments, text in another language, emoji only, an empty or
whitespace-only message.

```text
[es] U: asdkjh qwe
     T: No te he entendido.                    → unhandled

[en] U: blorp zzz
     T: I didn't understand that.              → unhandled

[es] U: 👍
     T: Vale.                                  → answered · no intent
     (a thumbs-up is an acknowledgement)

[en] U: ?
     T: I didn't understand that.              → unhandled

[es] U:
     T: No he recibido ningún texto.           → unhandled
     (empty or whitespace only)

[es] U: bonjour, quelle heure est-il
     T: Solo hablo español e inglés.           → unhandled
     (a third language is named as such and not guessed)
```

### 4.3 Generative requests

Anything that asks Talos to *produce* text of its own: a poem, a story, a
summary, an essay, an email, a plan, a joke made up on the spot, code, a
translation of a paragraph, advice composed to measure. Always `unhandled`,
never deflected, never "attempted" with a canned approximation.

```text
[es] U: escríbeme un poema sobre el mar
     T: No sé escribir textos nuevos.          → unhandled

[es] U: resúmeme este artículo: «El gobierno anunció...»
     T: No sé resumir textos.                  → unhandled

[es] U: hazme un plan de estudio para el examen del viernes
     T: No sé elaborar planes.                 → unhandled

[es] U: cuéntame un chiste que no hayas contado antes
     T: Eso no lo puedo hacer: no invento chistes.
                                               → unhandled

[en] U: write me a poem about the sea
     T: I can't write new text.                → unhandled

[en] U: summarize this paragraph for me: "The committee met on..."
     T: I can't summarise text.                → unhandled

[en] U: draft an email to my landlord
     T: I can't write emails.                  → unhandled

[en] U: give me a plan for learning Spanish in a month
     T: I can't make plans.                    → unhandled
```

Edges that are *not* generative, and stay with their own skill: "dime un
refrán" / "tell me a saying" (`fortune.tell`: a saying from a bundled list),
"traduce *perro* al inglés" / "how do you say *dog* in Spanish"
(`translate.word`), "qué significa *serendipia*" (`define.word`).

### 4.4 Requests for something that does not exist (yet)

Timers, alarms, reminders, calendar, e-mail, playing music, web browsing,
anything that would need Talos to speak first or touch the outside world,
and any capability not in the catalogue.

```text
[es] U: pon una alarma a las 7
     T: No sé poner alarmas.                   → unhandled

[es] U: recuérdame mañana llamar a mi madre
     T: No puedo recordarte cosas más tarde: solo hablo cuando me escribes.
                                               → unhandled

[en] U: set a timer for ten minutes
     T: I can't set timers.                    → unhandled

[en] U: play some jazz
     T: I can't play music.                    → unhandled
```

A skill whose capability is disabled (for instance a network skill when
network is off) answers like any other missing capability: "no puedo hacerlo
ahora" is acceptable, "no sé qué tiempo hace" is not an invention of an answer.

### 4.5 Questions about Talos that it cannot know

Talos answers about itself (`identity.who`, `identity.nature`) as defined in
[basics.md](basics.md). Questions that need knowledge it has not got ("quién
ganó la liga ayer", "what's the news today") are `unhandled` with a plain
"I don't know that": the reflector (ELIZA-style script) never answers
factual questions with a reflection that implies it knows.

### 4.6 A host that wants to pass the turn on

`Unhandled` is, by design, the signal a host (for example a chat front end
with a language model behind it) uses to pass the turn on. For that reason an
`unhandled` reply is **short, honest and does not end the conversation**:
the next turn is handled normally.

---

## 5. Determinism and reply variants

### 5.1 The guarantee

For the same **input** (text, language hint, `now`, caller place) and the same
**state** (conversation's dialog state, stored preferences and facts, installed
knowledge), Talos returns the same outcome: same status, intent, slots, and
the same reply text. There is no randomness and no reading of the clock
inside a turn; time is the `now` passed in.

Two consequences:

- A saved transcript replays identically. Examples in these specs are
  candidate golden transcripts.
- A test asserting a reply states its `now` and state, as the `Given:` lines do.

### 5.2 Variants

Several replies exist for most messages ("Son las 18:42." / "Las 18:42." in
Spanish). Which variant is used is a pure function of the **conversation and
the turn number** in it, never of chance and never of the clock. So:

- Two **different** turns of the same conversation may be worded differently
  although the content is the same, which keeps it from sounding like a
  machine.
- The **same turn number** of the same conversation always has the same wording.
- A **repeated question** ("qué hora es" twice) may get a variant on the
  second asking; it never gets different content.
- Variants never change the **content** (the number, the place, the
  language, the outcome). Tests assert content, outcome and language; the
  variant is illustrative (README, "Format of an exchange").

```text
Given: now = 2027-03-14 18:42 Europe/Madrid; a new conversation.

[es] U: qué hora es
     T: Son las 18:42.                         → answered · clock.now
[es] U: qué hora es
     T: Las 18:42.                             → answered · clock.now
     (same content, a different variant for turn 2)
```

### 5.3 Random skills

`random.*` skills do not break the guarantee: their "chance" is a function of
the conversation, the turn number and the text, so a replayed transcript
rolls the same dice. Within a conversation, asking twice in a row gives
independent-looking results because the turn number differs. The
configuration may add a seed of its own to make results unpredictable to a
user who tries to replay; the seed is then part of the installed state, and
with it fixed a replay is still exact. (The `Turn` carries no seed, so a host
cannot set one per turn.)

### 5.4 What is not deterministic

Nothing in a turn. The only things that change an answer between two runs of
the same text are a different `now`, a different caller place, or different
stored state (preferences taught or forgotten, facts taught or retracted).
