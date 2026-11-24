# State: `timezones` and `preferences`

Format as in [README.md](README.md); cross-cutting behaviour (language, slot
phrasings, open questions, cancellation, refusals) as in
[conversation.md](conversation.md). Both skills work offline.

Unless an exchange says otherwise:
`Given: now = 2027-03-14 18:42 Europe/Madrid` (a Sunday), no preferences
stored, no caller place.

Offsets used in the examples on that date (summer time has started in the
United States and Mexico does not observe it; Spain, the United Kingdom and
most of Europe change on 28 March; Australia changes on 4 April):

| Place | Offset from UTC | Local time at 18:42 Madrid |
|---|---|---|
| Madrid, Berlin | +1 | 18:42 |
| London, Lisbon, Canarias | 0 | 17:42 |
| New York | -4 | 13:42 |
| Chicago | -5 | 12:42 |
| Los Angeles | -7 | 10:42 |
| Mexico City | -6 | 11:42 |
| Bogotá, Lima | -5 | 12:42 |
| Buenos Aires | -3 | 14:42 |
| Tokyo | +9 | 02:42 (Monday) |
| Beijing | +8 | 01:42 (Monday) |
| Sydney | +11 | 04:42 (Monday) |
| Delhi | +5:30 | 23:12 |
| Dubai | +4 | 21:42 |

---

## Place memory, in one place

The remembered place (`preferences`, key `place`) is a value about the user,
stored on disk and kept across restarts. Several answers use it:

- **`timezones.time_in`**: "mi ciudad", "home", "where I live" mean the
  remembered place. ("Qué hora es aquí" / "what time is it here" needs only a
  zone and is `clock.now`.)
- **`timezones.difference`** with one place named: the other place is the
  remembered one.
- **`clock.now`, `clock.date`, `clock.weekday`, `clock.until`** with no place
  mentioned: the time zone of the remembered place is the reference zone (see
  [basics.md](basics.md)).
- **`calc.convert`** uses the remembered *units* to choose a target unit when
  the user does not name one.
- **Later skills** (`weather`) will default to the remembered place.

**Precedence of the reference place**, highest first:

1. A place named in the message.
2. The remembered place.
3. The place given by the caller with the turn (a host with GPS, say).
4. None. Skills that need a reference place and have none ask back (see the
   missing-slot exchanges below); skills that need only a zone use the
   server's zone and say nothing.

A remembered place changes nothing about what Talos says about *other*
places. It never makes Talos comment on where the user is.

---

## timezones

The current time in a place, and the difference in hours between two
places. Both use civil time zones and their daylight-saving rules for the date
`now`, from bundled data, so they work offline. They do not convert a stated
time ("when it is 9 in Madrid, what time is it in Bogotá?"), schedule a
meeting, or tell the weather.

### timezones.time_in

**Recognises** — a question about the current time in a named place: "what
time is it in X", "time in X", "tell me the time in X", with polite and
colloquial variants, a bare "¿y en X?" after a previous time or difference
question, and a request for several places in one sentence (up to three).
Also "mi ciudad"/"home" and similar references to the remembered place, and
the current date in a place ("qué día es en Tokio"). A bare "aquí"/"here"
("qué hora es aquí") is `clock.now`. Spanish and English
names of a place are the same place ([conversation.md](conversation.md),
section 2.4).

**Slots** — `place`, required, repeatable (up to 3). No default: the
remembered place is used only when the user refers to it ("mi ciudad",
"home"), never silently.

**Answers** — the local time (24-hour clock, `HH:MM`) in each place named. If
the local date differs from the date at the reference place, the reply says
so ("(ya es lunes)", "(still Sunday)"). When the place was a country answered
through its capital, the reply names the city it used. When the place was
ambiguous and the answers did not differ, it names the place it took (country
in brackets). Asked for the date there, it gives the local weekday and date
with the time.

**Does not** —
- "what time is it" with no place: `clock.now`.
- Converting a time the user gives ("si son las 9 en Madrid…"): not
  supported; `unhandled`, intent `timezones.time_in` kept.
- "How far ahead is X": `timezones.difference`.
- The weather or whether it is day or night in X: not supported.
- "Qué hora es aquí", "what time is it here": `clock.now`, which needs only a
  zone (precedence above).
- A date at the user's own place ("qué día es hoy"): `clock.date`. The date
  in a named place is this intent (cross-review resolution, see
  [decisions.md](decisions.md)).

#### Examples

```text
[es] U: ¿qué hora es en Tokio?
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio

[es] U: q hora es en nueva york
     T: En Nueva York son las 13:42.           → answered · timezones.time_in · place=Nueva York

[es] U: oye, dime la hora de Londres anda
     T: En Londres son las 17:42.              → answered · timezones.time_in · place=Londres

[es] U: ¿Qué hora es ahora mismo en Buenos Aires?
     T: En Buenos Aires son las 14:42.         → answered · timezones.time_in · place=Buenos Aires

[es] U: hora en pekin
     T: En Pekín es la 01:42 (ya es lunes).    → answered · timezones.time_in · place=Pekín

[es] U: ¿me puedes decir qué hora es en Sidney?
     T: En Sídney son las 04:42 (ya es lunes). → answered · timezones.time_in · place=Sídney
     (Sidney is accepted as a spelling of Sídney)

[es] U: ¿qué hora es en Lima y en Bogotá?
     T: En Lima y en Bogotá son las 12:42.     → answered · timezones.time_in · place=Lima,Bogotá
     (two places, same time: one sentence)

[en] U: what's the time in Tokyo?
     T: In Tokyo it's 02:42 (already Monday).  → answered · timezones.time_in · place=Tokyo

[en] U: what time is it in New York right now
     T: In New York it's 13:42.                → answered · timezones.time_in · place=New York

[en] U: time in london
     T: In London it's 17:42.                  → answered · timezones.time_in · place=London

[en] U: do you know what time it is in Sydney?
     T: In Sydney it's 04:42 (already Monday). → answered · timezones.time_in · place=Sydney

[en] U: tell me the time in Los Angeles please
     T: In Los Angeles it's 10:42.             → answered · timezones.time_in · place=Los Angeles

[en] U: what time is it in Mexico City?
     T: In Mexico City it's 11:42.             → answered · timezones.time_in · place=Mexico City

[en] U: current time in Delhi and Dubai
     T: In Delhi it's 23:12; in Dubai it's 21:42.
                                               → answered · timezones.time_in · place=Delhi,Dubai
```

A follow-up with only a place continues the previous question:

```text
[es] U: ¿qué hora es en Tokio?
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio
[es] U: ¿y en Londres?
     T: En Londres son las 17:42.              → answered · timezones.time_in · place=Londres

[en] U: what time is it in Dubai?
     T: In Dubai it's 21:42.                   → answered · timezones.time_in · place=Dubai
[en] U: and in Lima?
     T: In Lima it's 12:42.                    → answered · timezones.time_in · place=Lima
```

A missing place is asked back:

```text
[es] U: qué hora es en
     T: ¿En qué ciudad?                        → asks · place
[es] U: Tokio
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio

[en] U: what's the time over in
     T: In which city?                         → asks · place
[en] U: Sydney, thanks
     T: In Sydney it's 04:42 (already Monday). → answered · timezones.time_in · place=Sydney
```

A remembered place:

```text
Given: stored place = Sevilla (Europe/Madrid).

[es] U: ¿qué hora es en mi ciudad?
     T: En Sevilla son las 18:42.              → answered · timezones.time_in · place=Sevilla (remembered)

[en] U: what time is it at home?
     T: In Seville it's 18:42.                 → answered · timezones.time_in · place=Seville (remembered)
```

#### Edge cases

```text
[es] U: ¿qué hora es en Córdoba?
     T: ¿Córdoba de España o de Argentina?     → asks · choice
     (the two give different times, so Talos asks)
[es] U: la de Argentina
     T: En Córdoba (Argentina) son las 14:42.  → answered · timezones.time_in · place=Córdoba (AR)

[es] U: ¿qué hora es en Estados Unidos?
     T: ¿En qué ciudad? Hay varios husos horarios.
                                               → asks · place
     (no single zone represents the country)

[es] U: qué hora es en México
     T: En Ciudad de México son las 11:42.     → answered · timezones.time_in · place=México
     (one capital's zone stands for the country and the reply names the city)

[es] U: qué hora es en España
     T: En la España peninsular son las 18:42; en Canarias, las 17:42.
                                               → answered · timezones.time_in · place=España
     (Spain has two zones that both matter; Talos gives both, briefly)

[es] U: ¿qué hora es en Narnia?
     T: No conozco ese lugar.                  → unhandled · timezones.time_in
     (unknown place: an honest failure, not a guess)

[es] U: ¿qué hora es en UTC+2?
     T: En UTC+2 son las 19:42.                → answered · timezones.time_in · place=UTC+2
     (Madrid is UTC+1 on this date, so UTC+2 is one hour ahead)

[es] U: si en Madrid son las 9, ¿qué hora es en Tokio?
     T: Eso no sé calcularlo; solo sé la hora actual de cada sitio.
                                               → unhandled · timezones.time_in
     (conversion of a stated time is not in this skill)

[es] U: qué hora es aquí
     T: Son las 18:42.                         → answered · clock.now
     ("aquí" needs only a zone; with no remembered or caller place, the server's)

[es] U: ¿qué día es en Tokio?
     T: En Tokio ya es lunes, 15 de marzo (son las 02:42).
                                               → answered · timezones.time_in · place=Tokio

[es] U: ¿qué hora es allí?
     T: ¿En qué ciudad?                        → asks · place
     (nothing was named earlier in this conversation)

Given: stored place = Sevilla; earlier in the conversation, "Tokio" was named.

[es] U: ¿y allí qué hora es?
     T: En Tokio son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokio
     ("allí" is the last place named, not the remembered one)

[en] U: what time is it in nyc
     T: In New York it's 13:42.                → answered · timezones.time_in · place=New York

[en] U: what time is it in the US?
     T: In which city? The US has several time zones.
                                               → asks · place

[en] U: what time is it in Neverland
     T: I don't know that place.               → unhandled · timezones.time_in

[en] U: what time is it in Spain?
     T: Mainland Spain: 18:42. Canary Islands: 17:42.
                                               → answered · timezones.time_in · place=Spain

[en] U: what time is it in madrid and tokyo and london and sydney
     T: I can only tell you three places at a time. Which ones?
                                               → asks · place
     (more than three places: ask back rather than silently dropping some)

[en] U: what time is it in GMT?
     T: In GMT it's 17:42.                     → answered · timezones.time_in · place=GMT

[en] U: what time is it here?
     T: It's 18:42.                            → answered · clock.now
     (needs only a zone: the server's, said nothing about)

[en] U: what's the date in Sydney?
     T: In Sydney it's already Monday 15 March (04:42).
                                               → answered · timezones.time_in · place=Sydney

Given: caller place = Barcelona, no stored place.

[en] U: what time is it here?
     T: It's 18:42.                            → answered · clock.now
     (the caller's place sets the zone; precedence above)

[es] U: qué hora es in Tokyo
     T: En Tokyo son las 02:42 (ya es lunes).  → answered · timezones.time_in · place=Tokyo
     (Spanish grammar, English place name: the reply is Spanish, see
      conversation.md section 1.4)
```

### timezones.difference

**Recognises** — a question about the difference in hours between two
places ("difference between X and Y"), or between one place and the user's
place ("time difference with X"), including who is ahead or behind and yes/no
forms ("is X ahead of Y?").

**Slots** — `place` (`from`), `place` (`to`). Two places named: both used. One
named: it is the second, and the first is the reference place (precedence
above). The order in "entre X e Y" is irrelevant for the size of the
difference; for "is X ahead of Y" it fixes the direction.

**Answers** — the difference in hours (and minutes when not whole) *today*,
and which place is ahead. If the two places have the same time, it says so.
When an offset at either place is due to change within the next **30 days**
because of daylight saving, the reply adds what the difference will be after
the first such change, with its date. Hours are given as whole numbers and
"y media"/"and a half" or hours and minutes for odd offsets ("5 h 30 min").

**Does not** —
- The time itself in a place: `timezones.time_in`.
- A difference between dates or a duration: `clock.until`.
- A conversion of a stated time: unsupported.

#### Examples

```text
[es] U: ¿cuántas horas de diferencia hay entre Madrid y Tokio?
     T: Tokio va 8 horas por delante de Madrid. Desde el 28 de marzo serán 7.
                                               → answered · timezones.difference · place=Madrid,Tokio

[es] U: ¿qué diferencia horaria hay entre España y Argentina?
     T: Buenos Aires va 4 horas por detrás de Madrid. Desde el 28 de marzo serán 5.
                                               → answered · timezones.difference · place=España,Argentina

[es] U: ¿y entre Lima y Bogotá?
     T: Lima y Bogotá tienen la misma hora.    → answered · timezones.difference · place=Lima,Bogotá

[es] U: cuántas horas va Pekín por delante de Tokio
     T: Pekín va 1 hora por detrás de Tokio.   → answered · timezones.difference · place=Pekín,Tokio
     (the user's premise was wrong, and Talos corrects it without comment)

[es] U: ¿Sevilla y Madrid tienen la misma hora?
     T: Sí, tienen la misma hora.              → answered · timezones.difference · place=Sevilla,Madrid

[es] U: diferencia horaria entre Londres y Dubái
     T: Dubái va 4 horas por delante de Londres. Desde el 28 de marzo serán 3.
                                               → answered · timezones.difference · place=Londres,Dubái

[es] U: cuanta diferencia horaria hay entre delhi y dubai
     T: Delhi va 1 hora y media por delante de Dubái.
                                               → answered · timezones.difference · place=Delhi,Dubái

[en] U: what's the time difference between Madrid and Tokyo?
     T: Tokyo is 8 hours ahead of Madrid. From 28 March it will be 7.
                                               → answered · timezones.difference · place=Madrid,Tokyo

[en] U: how many hours ahead is Tokyo of London?
     T: Tokyo is 9 hours ahead of London. From 28 March it will be 8.
                                               → answered · timezones.difference · place=Tokyo,London

[en] U: how far behind is Los Angeles from Madrid
     T: Los Angeles is 8 hours behind Madrid. From 28 March it will be 9.
                                               → answered · timezones.difference · place=Los Angeles,Madrid

[en] U: time difference between London and Delhi
     T: Delhi is 5 hours 30 minutes ahead of London. From 28 March it will be 4 hours 30 minutes.
                                               → answered · timezones.difference · place=London,Delhi

[en] U: is Delhi ahead of Dubai?
     T: Yes, by 1 hour 30 minutes.             → answered · timezones.difference · place=Delhi,Dubai

[en] U: are Lima and Bogota on the same time?
     T: Yes, they have the same time.          → answered · timezones.difference · place=Lima,Bogotá

[en] U: Mexico City vs Chicago, what's the difference
     T: Chicago is 1 hour ahead of Mexico City.
                                               → answered · timezones.difference · place=Mexico City,Chicago
     (no daylight-saving change at either place in the next 30 days: no note)
```

One place named, with a remembered place:

```text
Given: stored place = Sevilla.

[es] U: ¿qué diferencia horaria hay con Nueva York?
     T: Nueva York va 5 horas por detrás de Sevilla. Desde el 28 de marzo serán 6.
                                               → answered · timezones.difference · place=Sevilla(remembered),Nueva York

[es] U: diferencia con Tokio
     T: Tokio va 8 horas por delante de Sevilla. Desde el 28 de marzo serán 7.
                                               → answered · timezones.difference · place=Sevilla(remembered),Tokio

[en] U: time difference with Sydney?
     T: Sydney is 10 hours ahead of Seville. From 28 March it will be 9; from 4 April, 8.
                                               → answered · timezones.difference · place=Seville(remembered),Sydney
```

(When there is a second change within the same 30 days, both are given.)

A missing place is asked back:

```text
[es] U: ¿cuántas horas de diferencia hay con?
     T: ¿Con qué lugar?                        → asks · place
[es] U: Bogotá
     T: ¿Respecto a qué lugar?                 → asks · place
[es] U: Madrid
     T: Bogotá va 6 horas por detrás de Madrid. Desde el 28 de marzo serán 7.
                                               → answered · timezones.difference · place=Madrid,Bogotá
```

Only the second place missing, nothing remembered:

```text
Given: no stored place, no caller place.

[es] U: ¿cuántas horas de diferencia hay con Bogotá?
     T: ¿Respecto a qué lugar?                 → asks · place
[es] U: Madrid
     T: Bogotá va 6 horas por detrás de Madrid. Desde el 28 de marzo serán 7.
                                               → answered · timezones.difference · place=Madrid,Bogotá

[en] U: what's the time difference with Tokyo?
     T: Compared to which place?               → asks · place
[en] U: london
     T: Tokyo is 9 hours ahead of London. From 28 March it will be 8.
                                               → answered · timezones.difference · place=London,Tokyo
```

#### Edge cases

```text
[es] U: diferencia horaria entre Madrid y Madrid
     T: Es el mismo lugar: no hay diferencia.  → answered · timezones.difference · place=Madrid,Madrid

[es] U: ¿cuál es la diferencia horaria entre Córdoba y Madrid?
     T: ¿Córdoba de España o de Argentina?     → asks · choice
[es] U: de España
     T: Córdoba y Madrid tienen la misma hora. → answered · timezones.difference · place=Córdoba (ES),Madrid

[es] U: diferencia horaria entre Narnia y Madrid
     T: No conozco Narnia.                     → unhandled · timezones.difference

[es] U: diferencia horaria entre Estados Unidos y España
     T: ¿Qué ciudad de Estados Unidos? Hay varios husos horarios.
                                               → asks · place

[es] U: diferencia entre Berlín y París
     T: Berlín y París tienen la misma hora.   → answered · timezones.difference · place=Berlín,París

[en] U: time difference with here
     T: Compared to which place? I don't know where you are.
                                               → asks · place

[en] U: what's the time difference between New York and New York?
     T: It's the same place, so there's no difference.
                                               → answered · timezones.difference · place=New York,New York

[en] U: how many hours ahead is London of Tokyo?
     T: London is 9 hours behind Tokyo; Tokyo is the one ahead. From 28 March it will be 8.
                                               → answered · timezones.difference · place=London,Tokyo
     (premise wrong; Talos states the true direction)

[en] U: what's the time difference between the US and Spain?
     T: Which US city? The US has several time zones.
                                               → asks · place

[en] U: time difference UTC+2 and Madrid
     T: UTC+2 is 1 hour ahead of Madrid. From 28 March they have the same time.
                                               → answered · timezones.difference · place=UTC+2,Madrid

[en] U: what's the time difference between Madrid and Tokyo and Sydney
     T: Which two places do you mean?          → asks · place
     (three places for a difference: ask, don't pick)
```

---

## preferences

What Talos remembers about the user across turns and restarts: **name**,
**place** (where the user is or lives), and **units** (metric or imperial).
Stored locally on disk, one set per installation (a single owner; see the
assumptions). Only these three
exist. Anything else the user asks to be remembered is declined.

Talos remembers only when asked. It never stores a preference on its own
initiative, never mentions one the user did not ask about, and the three
intents below are the only way to read, change or delete them. A change is
never made on classifier confidence alone (DESIGN.md §11): when
`preferences.set` or `preferences.forget` was recognised by the classifier
and not by a rule, Talos first asks a yes/no confirmation that restates the
change. The examples below assume a rule match.

### preferences.set

**Recognises** — a statement or request to remember one of the three. Name:
"me llamo…", "mi nombre es…", "soy…" followed by a name, "llámame…", "my name
is…", "call me…", "I'm…". Place: "vivo en…", "estoy en…", "mi ciudad es…",
"I live in…", "I'm in…", "my city is…", "set my location to…". Units: "uso el
sistema métrico", "prefiero millas", "prefiero grados Fahrenheit", "I prefer
metric", "use imperial units". And the bare "recuerda mi nombre"/"remember my
name" forms, where the value is missing. It is also what "recuerda que vivo en
Sevilla" means.

**Slots** — `key`: choice (`name`, `place`, `units`), required, normally
implied by the phrasing; `value`: `term` for a name, `place` for a place,
`choice` (`metric`, `imperial`) for units; required.

**Answers** — a confirmation that echoes what was stored, in the second
person. When replacing an existing value, it says the new value only. A name
is stored exactly as typed, but with a leading capital if the user wrote it
in lower case. A place that has several time zones or an ambiguous name is
resolved first (a question back); a place Talos does not know is **not**
stored.

**Does not** —
- Origin statements ("soy de Sevilla", "I'm from Seville") are not about where
  the user is: not `preferences.set`.
- "Soy"/"I'm" followed by anything that is not a name (an adjective, a
  profession, a nationality: "soy muy pesado", "I'm tired", "soy médico") is
  not a name. Talos does not store it, and does not comment on it.
- Anything other than name, place, units: "recuérdame mañana…" is a request
  Talos cannot do ([conversation.md](conversation.md) section 4.4); "háblame
  siempre en inglés" is unhandled: language follows the conversation.
  (Ruled 2026-11-24, ruling 11: refused for now.)
- Facts about the world: `teach.assert` when they come with a teaching cue
  ("aprende: los gatos son felinos"); without one they are ordinary
  statements, which go to the reflector and are not learnt
  ([knowledge.md](knowledge.md), teach).

#### Examples

```text
[es] U: me llamo Raúl
     T: Mucho gusto, Raúl. Lo recordaré.         → answered · preferences.set · key=name, value=Raúl

[es] U: vivo en Sevilla
     T: Vale, vives en Sevilla.                → answered · preferences.set · key=place, value=Sevilla

[es] U: oye, llámame Rulo
     T: Hecho, te llamaré Rulo.                → answered · preferences.set · key=name, value=Rulo

[es] U: estoy en Valencia, Venezuela
     T: Vale, estás en Valencia (Venezuela).   → answered · preferences.set · key=place, value=Valencia (VE)

[es] U: prefiero unidades métricas
     T: Anotado: unidades métricas.            → answered · preferences.set · key=units, value=metric

[es] U: yo uso millas y fahrenheit, no kilómetros
     T: Anotado: unidades imperiales.          → answered · preferences.set · key=units, value=imperial

[es] U: mi nombre es ana maría
     T: Mucho gusto, Ana María. Lo recordaré.    → answered · preferences.set · key=name, value=Ana María
     (the capitals are added)

[es] U: recuerda que vivo en Madrid
     T: Vale, vives en Madrid.                 → answered · preferences.set · key=place, value=Madrid

[en] U: my name is Sam
     T: Nice to meet you, Sam. I'll remember that.
                                               → answered · preferences.set · key=name, value=Sam

[en] U: call me Sammy
     T: Okay, I'll call you Sammy.             → answered · preferences.set · key=name, value=Sammy

[en] U: I live in Bristol
     T: Got it, you live in Bristol.           → answered · preferences.set · key=place, value=Bristol

[en] U: i'm in new york
     T: Got it, you're in New York.            → answered · preferences.set · key=place, value=New York

[en] U: I prefer metric
     T: Noted: metric units.                   → answered · preferences.set · key=units, value=metric

[en] U: use imperial units please
     T: Noted: imperial units.                 → answered · preferences.set · key=units, value=imperial

[en] U: remember that I live in Leeds
     T: Got it, you live in Leeds.             → answered · preferences.set · key=place, value=Leeds
```

Replacing a value:

```text
Given: stored place = Sevilla.

[es] U: ahora vivo en Barcelona
     T: Vale, vives en Barcelona.              → answered · preferences.set · key=place, value=Barcelona

[en] U: I moved to Edinburgh
     T: Got it, you're in Edinburgh.           → answered · preferences.set · key=place, value=Edinburgh
```

With a missing value, Talos asks:

```text
[es] U: recuerda mi nombre
     T: ¿Cómo te llamas?                       → asks · term (value of name)
[es] U: Raúl
     T: Mucho gusto, Raúl. Lo recordaré.         → answered · preferences.set · key=name, value=Raúl

[es] U: apunta dónde vivo
     T: ¿Dónde vives?                          → asks · place
[es] U: en Sevilla
     T: Vale, vives en Sevilla.                → answered · preferences.set · key=place, value=Sevilla

[en] U: remember my name
     T: What's your name?                      → asks · term (value of name)
[en] U: it's Sam
     T: Nice to meet you, Sam. I'll remember that.
                                               → answered · preferences.set · key=name, value=Sam

[en] U: set my units
     T: Metric or imperial?                    → asks · choice
[en] U: metric
     T: Noted: metric units.                   → answered · preferences.set · key=units, value=metric

[es] U: configura mis unidades
     T: ¿Métricas o imperiales?                → asks · choice
[es] U: las métricas
     T: Anotado: unidades métricas.            → answered · preferences.set · key=units, value=metric

[es] U: recuerda mi ciudad
     T: ¿En qué ciudad vives?                  → asks · place
[es] U: déjalo
     T: Vale, lo dejo.                         → answered · no intent · expectation cancelled
     (nothing is stored)
```

#### Edge cases

```text
[es] U: soy de Sevilla
     T: De Sevilla, entonces.                  → deflected · self
     (an origin is not a place of residence: not preferences.set, nothing
      stored; the reflector's S5 takes it)

[en] U: I'm from Seville
     T: From Seville, then.                    → deflected · self

[es] U: soy Raúl
     T: Mucho gusto, Raúl. Lo recordaré.         → answered · preferences.set · key=name, value=Raúl

[es] U: soy muy pesado
     T: Muy pesado, entonces.                  → deflected · self
     (not a name; nothing stored; the reflector's S5)

[en] U: I'm tired
     T: Ah. Not much I can do about that, but I'm still here.
                                               → deflected · self
     (not a name; nothing stored)

[es] U: vivo en Narnia
     T: No conozco ese lugar, así que no lo he guardado.
                                               → unhandled · preferences.set
     (not stored)

[es] U: vivo en Córdoba
     T: ¿Córdoba de España o de Argentina?     → asks · choice
[es] U: la de España
     T: Vale, vives en Córdoba (España).       → answered · preferences.set · key=place, value=Córdoba (ES)

[es] U: vivo en Estados Unidos
     T: ¿En qué ciudad? Hay varios husos horarios.
                                               → asks · place
[es] U: Chicago
     T: Vale, vives en Chicago.                → answered · preferences.set · key=place, value=Chicago

[es] U: recuerda que mi color favorito es el rojo
     T: Solo recuerdo tu nombre, tu lugar y tus unidades.
                                               → unhandled · preferences.set
     (an unknown key is a plain refusal, not stored)

[es] U: háblame siempre en inglés
     T: Respondo en el idioma en que me escribes.
                                               → unhandled

[en] U: always speak Spanish to me
     T: I answer in the language you write in.
                                               → unhandled

[en] U: my name is "Dr. Sam O'Neil"
     T: Nice to meet you, Dr. Sam O'Neil. I'll remember that.
                                               → answered · preferences.set · key=name, value=Dr. Sam O'Neil
     (quotes are removed; the capitals and punctuation are kept)

[en] U: my name is
     T: What's your name?                      → asks · term
[en] U: never mind
     T: Okay, dropped.                         → answered · no intent · expectation cancelled

[es] U: me llamo Talos
     T: Mucho gusto, Talos. Lo recordaré.      → answered · preferences.set · key=name, value=Talos
     (stored as asked; Talos does not comment on the coincidence)
```

### preferences.get

**Recognises** — a question about what Talos remembers: "cómo me llamo", "qué
sabes de mí", "dónde vivo", "qué unidades uso", "what's my name", "what do you
know about me", "where do I live", "what units do I use".

**Slots** — `key`: choice (`name`, `place`, `units`), optional. With no key,
all stored preferences are listed.

**Answers** — the stored value, or all stored values in one sentence. If
nothing is stored for the key, Talos says it does not know it. It never asks
the user to supply it (that would be an unsolicited suggestion), and it never
makes up a value from the caller's place: the caller place is not a
preference.

**Does not** —
- "Qué sabes de X" about a thing (not "de mí"): `relations.about`.
- "Quién soy" in a philosophical sense: reflector.
- "Cómo te llamas": `identity.who`.

#### Examples

```text
Given: stored name = Raúl, place = Sevilla, units = metric.

[es] U: ¿cómo me llamo?
     T: Te llamas Raúl.                        → answered · preferences.get · key=name

[es] U: ¿dónde vivo?
     T: Vives en Sevilla.                      → answered · preferences.get · key=place

[es] U: ¿qué unidades uso?
     T: Unidades métricas.                     → answered · preferences.get · key=units

[es] U: ¿qué sabes de mí?
     T: Te llamas Raúl, vives en Sevilla y usas unidades métricas.
                                               → answered · preferences.get · key=all

[es] U: oye ¿te acuerdas de cómo me llamo?
     T: Sí, Raúl.                              → answered · preferences.get · key=name

[es] U: a ver, ¿dónde estoy según tú?
     T: En Sevilla.                            → answered · preferences.get · key=place

[es] U: ¿sabes mi nombre?
     T: Sí: Raúl.                              → answered · preferences.get · key=name

[es] U: dime todo lo que recuerdas de mí
     T: Te llamas Raúl, vives en Sevilla y usas unidades métricas.
                                               → answered · preferences.get · key=all

[en] U: what's my name?
     T: Your name is Raúl.                     → answered · preferences.get · key=name

[en] U: where do I live?
     T: You live in Seville.                   → answered · preferences.get · key=place

[en] U: which units do I use
     T: Metric units.                          → answered · preferences.get · key=units

[en] U: what do you know about me?
     T: Your name is Raúl, you live in Seville and you use metric units.
                                               → answered · preferences.get · key=all

[en] U: do you remember my name?
     T: Yes, it's Raúl.                        → answered · preferences.get · key=name

[en] U: what do you remember about me
     T: Your name is Raúl, you live in Seville and you use metric units.
                                               → answered · preferences.get · key=all

[en] U: where am I, according to you?
     T: In Seville.                            → answered · preferences.get · key=place

[en] U: remind me, what's my name again
     T: Raúl.                                  → answered · preferences.get · key=name
```

Nothing stored:

```text
Given: nothing stored.

[es] U: ¿cómo me llamo?
     T: No sé cómo te llamas.                  → answered · preferences.get · key=name

[es] U: ¿qué sabes de mí?
     T: Todavía no sé nada de ti.              → answered · preferences.get · key=all

[en] U: where do I live?
     T: I don't know where you live.           → answered · preferences.get · key=place

[en] U: what do you know about me?
     T: I don't know anything about you yet.   → answered · preferences.get · key=all
```

(The status is `answered`: "I don't know" is the true answer to the question
asked, and the skill produced it. It is not `unhandled`.)

#### Edge cases

```text
Given: stored place = Sevilla; caller place = Barcelona.

[es] U: ¿dónde estoy?
     T: En Sevilla.                            → answered · preferences.get · key=place
     (stored wins over the caller's)

Given: no stored place; caller place = Barcelona.

[es] U: ¿dónde vivo?
     T: No sé dónde vives.                     → answered · preferences.get · key=place
     (a host's location is not a preference Talos remembers)

Given: stored name = Raúl only.

[es] U: ¿qué sabes de mí?
     T: Solo sé que te llamas Raúl.            → answered · preferences.get · key=all

[en] U: what do you know about me?
     T: Only that your name is Raúl.           → answered · preferences.get · key=all

[es] U: ¿qué sabes de Sevilla?
     T: (a question about a thing)             → relations.about, not preferences.get

[es] U: ¿cómo te llamas?
     T: (a question about Talos)               → identity.who, not preferences.get

[en] U: who am I?
     T: Your name is Raúl.                     → answered · preferences.get · key=name
     (taken at face value: the only thing Talos knows is the name)

Given: nothing stored.

[en] U: who am I?
     T: I don't know who you are.              → answered · preferences.get · key=name

[es] U: ¿cuál es mi color favorito?
     T: No recuerdo eso: solo guardo tu nombre, tu lugar y tus unidades.
                                               → unhandled · preferences.get
```

### preferences.forget

**Recognises** — a request to forget one preference or all: "olvida mi
nombre", "borra mi ciudad", "ya no vivo en Sevilla", "olvídate de mí", "borra
todo lo que sabes de mí", "forget my name", "delete my location", "forget
everything about me", "clear my preferences".

**Slots** — `key`: choice (`name`, `place`, `units`, `all`), required. With a
named key Talos forgets at once. For `all`, or when the phrasing does not
make clear what to delete ("olvida lo que sabes"), Talos asks to confirm,
because the deletion is permanent. A bare "olvida"/"forget" is asked back as
a choice ("¿Qué olvido: tu nombre, tu lugar o tus unidades?").

**Answers** — a confirmation of what was deleted. If nothing was stored for
that key, Talos says so; the state is the same.

**Does not** —
- Retracting a taught fact ("olvida que los gatos son felinos"): `teach.forget`.
- Cancelling an open question ("olvídalo", "forget it"): a cancellation when a
  question is open ([conversation.md](conversation.md) section 3.5).
- Forgetting the conversation so far: not supported; `unhandled`.

#### Examples

```text
Given: stored name = Raúl, place = Sevilla, units = metric.

[es] U: olvida mi nombre
     T: Hecho, ya no recuerdo tu nombre.       → answered · preferences.forget · key=name

[es] U: borra mi ciudad
     T: Hecho, ya no recuerdo dónde vives.     → answered · preferences.forget · key=place

[es] U: ya no quiero que recuerdes mis unidades
     T: Hecho, ya no recuerdo tus unidades.    → answered · preferences.forget · key=units

[es] U: olvida todo lo que sabes de mí
     T: ¿Seguro? Borraré tu nombre, tu lugar y tus unidades.
                                               → asks · choice (yes/no)
[es] U: sí
     T: Hecho. Ya no recuerdo nada sobre ti.   → answered · preferences.forget · key=all

[es] U: olvídame
     T: ¿Seguro? Borraré tu nombre, tu lugar y tus unidades.
                                               → asks · choice (yes/no)
[es] U: no
     T: Vale, no borro nada.                   → answered · preferences.forget · nothing deleted
     (a "no" answers the question; the intent is kept, conversation.md 3.3)

[es] U: ya no vivo en Sevilla
     T: Hecho, ya no recuerdo dónde vives.     → answered · preferences.forget · key=place
     (a statement of change that does not name a new place)

[es] U: borra mi nombre porfa
     T: Hecho, ya no recuerdo tu nombre.       → answered · preferences.forget · key=name

[en] U: forget my name
     T: Done, I no longer remember your name.  → answered · preferences.forget · key=name

[en] U: delete my location
     T: Done, I no longer remember where you live.
                                               → answered · preferences.forget · key=place

[en] U: stop remembering my units
     T: Done, I no longer remember your units. → answered · preferences.forget · key=units

[en] U: forget everything about me
     T: Are you sure? I'll delete your name, location and units.
                                               → asks · choice (yes/no)
[en] U: yes
     T: Done. I no longer remember anything about you.
                                               → answered · preferences.forget · key=all

[en] U: clear my preferences
     T: Are you sure? I'll delete your name, location and units.
                                               → asks · choice (yes/no)
[en] U: nope
     T: Okay, nothing deleted.                 → answered · preferences.forget · nothing deleted

[en] U: I don't live in Bristol any more
     T: Done, I no longer remember where you live.
                                               → answered · preferences.forget · key=place

[en] U: please forget my name
     T: Done, I no longer remember your name.  → answered · preferences.forget · key=name
```

A bare request is asked back:

```text
[es] U: olvida
     T: ¿Qué olvido: tu nombre, tu lugar o tus unidades?
                                               → asks · choice
[es] U: el lugar
     T: Hecho, ya no recuerdo dónde vives.     → answered · preferences.forget · key=place

[en] U: forget
     T: What should I forget: your name, your location or your units?
                                               → asks · choice
[en] U: the first one
     T: Done, I no longer remember your name.  → answered · preferences.forget · key=name
```

How a forgotten place changes other answers:

```text
Given: stored place = Sevilla.

[es] U: qué hora es en mi ciudad
     T: En Sevilla son las 18:42.              → answered · timezones.time_in · place=Sevilla (remembered)
[es] U: olvida mi ciudad
     T: Hecho, ya no recuerdo dónde vives.     → answered · preferences.forget · key=place
[es] U: qué hora es en mi ciudad
     T: ¿En qué ciudad estás?                  → asks · place
[es] U: Sevilla
     T: En Sevilla son las 18:42.              → answered · timezones.time_in · place=Sevilla
     (the place is used for this answer only; it is not remembered again)

[en] U: remember that I live in Leeds
     T: Got it, you live in Leeds.             → answered · preferences.set · key=place, value=Leeds
[en] U: time difference with Tokyo?
     T: Tokyo is 9 hours ahead of Leeds. From 28 March it will be 8.
                                               → answered · timezones.difference · place=Leeds(remembered),Tokyo
[en] U: forget my location
     T: Done, I no longer remember where you live.
                                               → answered · preferences.forget · key=place
[en] U: time difference with Tokyo?
     T: Compared to which place?               → asks · place
```

#### Edge cases

```text
Given: nothing stored.

[es] U: olvida mi nombre
     T: No tenía guardado tu nombre.           → answered · preferences.forget · key=name

[en] U: forget everything about me
     T: I don't have anything stored about you.
                                               → answered · preferences.forget · key=all
     (nothing to confirm: no question is asked)

Given: stored name = Raúl.

[es] U: olvida todo lo que sabes de mí
     T: ¿Seguro? Borraré tu nombre.            → asks · choice (yes/no)
[es] U: venga
     T: Hecho. Ya no recuerdo nada sobre ti.   → answered · preferences.forget · key=all
     ("venga" is yes when a yes/no is open)

[es] U: olvida todo lo que sabes de mí
     T: ¿Seguro? Borraré tu nombre.            → asks · choice (yes/no)
[es] U: cuánto es 2 más 2
     T: 4.                                     → answered · calc.arithmetic
     (an unrelated message releases the question: nothing was deleted)
[es] U: sí
     T: No tengo ninguna pregunta pendiente.   → unhandled
     (and the question is closed; the name is still stored)

[en] U: forget everything about me
     T: Are you sure? I'll delete your name.   → asks · choice (yes/no)
[en] U: never mind
     T: Okay, nothing deleted.                 → answered · no intent · expectation cancelled

[es] U: olvida que los gatos son felinos
     T: (a taught fact)                        → teach.forget, not preferences.forget

[en] U: forget that cats are mammals
     T: (a taught fact)                        → teach.forget, not preferences.forget

[es] U: olvídalo
     T: Vale.                                  → answered · no intent
     (nothing open and no object: a bare cancellation, not a deletion)

[en] U: forget our conversation
     T: I can't forget the conversation.       → unhandled
     (the conversation is not a stored preference)

[es] U: olvida mi color favorito
     T: Solo guardo tu nombre, tu lugar y tus unidades.
                                               → unhandled · preferences.forget
```

---

## Preferences feeding other answers: a longer exchange

```text
Given: now = 2027-03-14 18:42 Europe/Madrid; nothing stored.

[es] U: qué hora es en mi ciudad
     T: ¿En qué ciudad estás?                  → asks · place
[es] U: en Sevilla
     T: En Sevilla son las 18:42.              → answered · timezones.time_in · place=Sevilla
[es] U: apunta que vivo ahí
     T: Vale, vives en Sevilla.                → answered · preferences.set · key=place, value=Sevilla
     ("ahí" is the place last named in the conversation)
[es] U: diferencia horaria con Tokio
     T: Tokio va 8 horas por delante de Sevilla. Desde el 28 de marzo serán 7.
                                               → answered · timezones.difference · place=Sevilla(remembered),Tokio
[es] U: ¿y con Nueva York?
     T: Nueva York va 5 horas por detrás de Sevilla. Desde el 28 de marzo serán 6.
                                               → answered · timezones.difference · place=Sevilla(remembered),Nueva York

[en] U: set my location to Bristol
     T: Got it, you're in Bristol.             → answered · preferences.set · key=place, value=Bristol
[en] U: what time is it at home?
     T: In Bristol it's 17:42.                 → answered · timezones.time_in · place=Bristol (remembered)
[en] U: how far ahead is Madrid?
     T: Madrid is 1 hour ahead of Bristol.     → answered · timezones.difference · place=Bristol(remembered),Madrid
[en] U: and Tokyo?
     T: Tokyo is 9 hours ahead of Bristol. From 28 March it will be 8.
                                               → answered · timezones.difference · place=Bristol(remembered),Tokyo
```

Units used by `calc.convert` when no target is named (illustrative; the
normative text is in [basics.md](basics.md)):

```text
Given: stored units = metric.

[es] U: convierte 10 millas
     T: 10 millas son ≈ 16,09 km.              → answered · calc.convert
     (rounded, so marked ≈ as in basics.md)

Given: stored units = imperial.

[en] U: convert 10 kilometres
     T: 10 km is ≈ 6.21 mi.                    → answered · calc.convert
```

---

## Assumptions made where the design is silent

1. **Remembered place vs the caller's place.** The turn carries an optional
   caller place (`Turn.place`) and `preferences` stores a place. The design
   does not say which wins. Assumed: what the user told Talos beats what the
   host guessed. **Ruled (ruling 5, [decisions.md](decisions.md)).**
2. **Who a preference belongs to.** One set per installation, with a single
   owner (DESIGN.md non-goal: multi-user accounts). Nothing is keyed on the
   API `user` field or on an identified user: every conversation of the
   installation reads and writes the same name, place and units. **Ruled
   (ruling 10).**
3. **A remembered place is one value**, whether the user said where they
   live or where they are now; the last statement wins.
4. **Mixed-language turns** ("qué hora es in Tokyo") follow the grammar, not the place name: Spanish.
5. **The `unhandled` status for an unknown place** although the intent was
   recognised: an honest failure is `unhandled` with the intent kept.
6. **Conversion of a stated time** ("si son las 9 en Madrid…") is treated as
   outside `timezones`, since the catalogue lists only the two intents.
7. **"Aquí"/"here" and the date in a place** (cross-review): "qué hora es
   aquí" is `clock.now`, which needs only a zone; "qué día es en Tokio" is
   `timezones.time_in`, which gives the local date with the time. With no
   place known, "aquí"/"here" uses the machine's configured zone without
   asking. **Ruled (ruling 6).**
