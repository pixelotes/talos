# Decisions taken in the specification

Every behaviour the specification files decided where the design documents
([DESIGN.md](../DESIGN.md), [KNOWLEDGE.md](../KNOWLEDGE.md),
[SKILLS.md](../SKILLS.md), [API.md](../API.md), the ADRs) were silent, plus the
resolutions taken when the five files were cross-reviewed against each other.
It consolidates the lists at the end of [basics.md](basics.md),
[state.md](state.md), [knowledge.md](knowledge.md) and
[reflector.md](reflector.md), and the decisions [conversation.md](conversation.md)
takes without listing them.

Each entry gives the decision, where it lives, and a classification:

- **needs owner ruling** — product-level, sensitive or expensive to reverse;
  listed first, numbered, phrased so it can be answered yes/no or by picking
  an option. The option the specification currently follows is marked *(as
  specified)*.
- **reasonable default** — noted only.
- **conflicts with design docs** — the specification departs from, or goes
  beyond, a design document; the document is named.

---

## Rulings (2026-11-24)

The owner ruled on all fourteen questions that were open. Each entry gives the
question, the ruling and where it is specified.

1. **Distress reply.** Keep the fixed reply with emergency numbers, status
   `unhandled`? **Ruling:** keep; the numbers are configuration by the user's
   place, default Spain 112 and 024, generic "your local emergency number"
   wording when no place is known, no hard-coded 112/999/911 mix. —
   reflector.md S29, decision 7; conversation.md 3.9, 4.1.
2. **Where the distress check runs.** Before the pending-dialog stage?
   **Ruling:** yes; an open yes/no can never swallow a distress message
   (amends [ADR 0004](../adr/0004-explicit-turn-pipeline.md)). —
   conversation.md 3.9; reflector.md 2.1, S29.
3. **Confidence wording bands.** Accept 0.85 / 0.60 / 0.30, decay 0.99 per
   link, 0.95 taught? **Ruling:** accepted as initial values that may be
   tuned. — knowledge.md 1.5, 2.5, decisions 3–4.
4. **Who created Talos.** **Ruling:** changed: answered
   (`answered · identity.who`) with a fixed, configurable identity line; the
   default says Talos is a project by Pixelotes, naming no individual and no
   other company. Origin of the name and age stay `unhandled`. — basics.md
   identity.who, assumptions.
5. **Remembered place vs caller place.** **Ruling:** the remembered place
   wins. — state.md "Place memory", assumption 1.
6. **"Aquí"/"here" with no place known.** **Ruling:** use the machine's
   configured zone without asking. — state.md assumption 7; conversation.md
   2.4.
7. **English conventions.** **Ruling:** 24-hour clock and day before month;
   a user preference is a later addition. — basics.md conventions and
   assumptions; conversation.md 2.3.
8. **A definition that cannot be given.** **Ruling:** a word with no gloss
   but with relations to offer is `unhandled · define.word`. — knowledge.md
   define.word, decision 11.
9. **Word-pair lexicon answers.** **Ruling:** `answered`, hedged in words, up
   to two candidates. — knowledge.md translate.word, decision 24.
10. **Whose preferences.** **Ruling:** one set per installation (single
    owner); nothing keyed on the API `user` field or an identified user. —
    state.md preferences, assumption 2.
11. **Language preference.** **Ruling:** "háblame siempre en inglés" is
    refused for now. — state.md preferences.set.
12. **Other languages of Spain.** **Ruling:** Catalan, Galician and Basque get
    the plain "only Spanish and English" reply, language unnamed. —
    reflector.md 4.5, decision 14; conversation.md 4.2.
13. **Tú only.** **Ruling:** always tú, even when addressed as usted. —
    reflector.md 3.2, decision 9.
14. **Status of dialog-level replies.** **Ruling:** acknowledgements,
    cancellations and hedges are `answered` with no intent; the design docs
    are being amended to match. — conversation.md 1.2, 3.5, 3.7.

---

## Status and outcomes

| Decision | Where | Class |
|---|---|---|
| Impossible but well-formed requests (÷0, incompatible units, 31 April, below absolute zero, 10 distinct numbers from 5) are `answered`; unsupported ones (unknown unit, too large, unsupported dice) `unhandled` with the intent kept. | basics.md conventions | reasonable default (now README's rule) |
| Answers resting on absence (unknown word or place, nothing known, beyond depth, no target word) are `unhandled`, intent kept; contradicted "no" is `answered`; empty `teach.list` and "nothing to forget" are `answered`. | knowledge.md 2.4, 2.9, decision 1 | reasonable default |
| `preferences.get` with nothing stored ("No sé cómo te llamas") is `answered`. | state.md preferences.get | reasonable default |
| A refused deleting or unsupported teaching shape, and a refused `teach.phrase`, are `unhandled` with the intent. | knowledge.md teach | reasonable default |
| "Are you sure?" about the clock's answer is `answered · relations.provenance` (cross-review: was `unhandled`). | knowledge.md provenance | reasonable default |
| Advice (S27), commands Talos cannot do (S28) and distress (S29) are recognised shapes but `unhandled`. | reflector.md decision 2 | reasonable default |
| Shape ids and unhandled reasons are carried in `Outcome.reason` and in the API's `talos.reason`, as well as in the trace; `intent` stays empty for reflector turns. | reflector.md 2.2, decision 3 | resolved 2026-11-24: design amended (DESIGN.md §3, API.md) |
| Skills return honest `unhandled` replies (unknown word, unknown place). | all skill files | resolved 2026-11-24: design amended (`SkillResult::Unable`, DESIGN.md §3, ADR 0009, SKILLS.md §6) |
| Outcome lines list slots. | README format, all files | resolved 2026-11-24: `Outcome` carries typed slots (DESIGN.md §3) |
| A "no" to a yes/no keeps the original intent (`answered · preferences.forget`); a cancellation has no intent (cross-review: state.md had "no intent" for "no"). | conversation.md 3.3; state.md; knowledge.md 2.9 | reasonable default |

## Language and conventions

| Decision | Where | Class |
|---|---|---|
| A caller's hint wins; otherwise detection with stickiness; one stray word ("gracias" in English) never switches; a full phrase does, from that turn on. | conversation.md 1.1–1.3 | reasonable default |
| Mixed input: the language is that of the grammar (function words, verb); loanwords fill slots. | conversation.md 1.4; state.md assumption 4 | reasonable default |
| Third languages are recognised as such and refused in the conversation language. | conversation.md 2.5, 4.2; reflector.md 4.5 | reasonable default (see ruling 12) |
| Number separators by language: Spanish dot = thousands only before exactly three digits, comma decimal; English the reverse; malformed or mixed are asked; a lone comma not followed by three digits in English is read as a decimal and the reading shown. | conversation.md 2.1; basics.md conventions | reasonable default |
| Output grouping: Spanish from five digits (1750, 12.345), English from four (1,750) (cross-review: basics.md prose said English from five, its examples from four; conversation.md grouped 1.750). | basics.md; conversation.md 2.1 | reasonable default |
| "Billón" is 10^12 in Spanish, "billion" 10^9 in English. | basics.md | reasonable default |
| Rounded values are always marked `≈` (cross-review: state.md's conversion examples lacked it). | basics.md; state.md | reasonable default |
| Spanish clock 24-hour, "Es la 1:05" singular; midnight/noon marked. | basics.md clock.now | reasonable default |
| English clock, date order and numeric dates. | basics.md; conversation.md 2.3 | see ruling 7 |
| Talos never uses a gendered adjective about itself or the user ("encantado", "no estoy seguro"); knowledge.md's weak band now says «no lo sé con certeza». | reflector.md 3.2; knowledge.md 2.5, decision 25; basics.md | reasonable default |

## Open questions, cancellation, yes/no

| Decision | Where | Class |
|---|---|---|
| At most one question at a time; never asks for an optional slot. | conversation.md 3.1 | reasonable default |
| An expectation expires after 3 further turns (hedges and invalid values count) or 5 minutes, silently. | conversation.md 3.7 | reasonable default |
| Never the same question a third time: after two failed answers it is dropped, `unhandled` with the intent. | conversation.md 3.4 | reasonable default |
| An unrelated message releases **and closes** the question; no reminder. A message that is a certain new intent wins even if it contains a value. | conversation.md 3.6 | reasonable default |
| A value plus a separate new request: both answered only when clearly two sentences. | conversation.md 3.2 | reasonable default |
| Cancellation words (lists in 3.5) cancel only when something is open; otherwise a minimal acknowledgement. | conversation.md 3.5 | reasonable default |
| "Vale", "venga", "dale" are yes only while a yes/no is open; "no, gracias" is a "no". | conversation.md 3.3; basics.md smalltalk.thanks | reasonable default |
| Bare "sí"/"no"/"yes", "sí, por favor", "no, gracias" with nothing open: "no pending question", `unhandled`, no intent (cross-review: basics.md had `deflected` for "no, gracias"). | conversation.md 3.8; basics.md | reasonable default |
| Acknowledgements ("ok", "vale", "genial", 👍) with nothing open get a minimal acknowledgement and never a question. | conversation.md 1.2, 4.2; reflector.md 2.1 | see ruling 14 |
| Choice by name, position, unique part; "ninguna"/"none"/"ni idea" closes. | conversation.md 2.7 | reasonable default |

## Determinism and randomness

| Decision | Where | Class |
|---|---|---|
| Reply variants are a pure function of conversation and turn number; content never varies. | conversation.md 5.2; reflector.md 6 | reasonable default (DESIGN.md §4 stage 6) |
| Random draws are a pure function of conversation, turn number and text, plus an optional seed in configuration; never system entropy (cross-review: basics.md seeded from entropy, against DESIGN.md §1 goal 2; conversation.md let a host pass a seed, which `Turn` cannot carry). | conversation.md 5.3; basics.md "Randomness" | reasonable default; the configured seed is not in DESIGN.md §10 |
| The reflector never repeats a reply text in consecutive turns; variants cycle within a shape; hand-back questions thin out after two. | reflector.md 6 | reasonable default |

## Places and time zones

| Decision | Where | Class |
|---|---|---|
| Place precedence. | state.md | see ruling 5 |
| "Mi ciudad"/"home" = remembered place; "aquí"/"here" = where the user is (remembered, else caller); "allí"/"there" = last place named (cross-review: "qué hora es aquí" is `clock.now`; state.md had it as `timezones.time_in` asking back). | conversation.md 2.4; state.md; basics.md clock.now | see ruling 6 |
| The date in a named place ("qué día es en Tokio") is `timezones.time_in`, answered with local time and date (cross-review: basics.md and state.md each sent it to the other). | state.md; basics.md clock.date | reasonable default |
| A country with several zones is asked as a city; one capital stands for a country; Spain answers both peninsular and Canary time. | conversation.md 2.4; state.md | reasonable default |
| Ambiguous names are asked only when the candidates' answers differ. | conversation.md 2.4; state.md | reasonable default |
| Nicknames ("la Gran Manzana") are not recognised; one-letter typos on long names are. | conversation.md 2.4 | reasonable default |
| `timezones.time_in` takes up to three places; more is asked back. | state.md | reasonable default |
| `timezones.difference` adds the change due to daylight saving within 30 days. | state.md | reasonable default |
| Converting a stated time ("si son las 9 en Madrid…") is unsupported, `unhandled · timezones.time_in`. | state.md assumption 6 | reasonable default |
| A remembered place is one value; the last statement wins; an origin ("soy de Sevilla") is not stored and goes to the reflector (S5). | state.md assumption 3 | reasonable default |

## Clock and calendar

| Decision | Where | Class |
|---|---|---|
| A bare weekday is the next one after today (seven days ahead when it is today); "este"/"this" includes today (cross-review: basics.md had "today included" for the bare weekday too). | conversation.md 2.3; basics.md clock | reasonable default |
| "Next Friday" is the first Friday strictly after today; the resolved date is always stated. | basics.md clock, assumptions | reasonable default |
| A bare hour is the next time that reading occurs; a passed time of day means tomorrow. | conversation.md 2.3; basics.md clock.until | reasonable default |
| Day parts are ranges with representative times (09:00, 12:00, 17:00, 21:00, 03:00). | conversation.md 2.3 | reasonable default |
| Two-digit years are 20xx; a date without a year is the next occurrence ("since" takes the last one). | conversation.md 2.3; basics.md | reasonable default |
| `clock.date` also answers relative days, `clock.now` an offset. | basics.md assumptions | resolved 2026-11-24 by amending the design; was a conflict: README's catalogue says "today's date" and "the current time" only |
| Well-known days: a fixed list (Año Nuevo, Reyes, San Valentín, Halloween, Nochebuena, Navidad, Nochevieja, Easter computed). | basics.md clock.until | reasonable default |
| Dates before 1583 in the proleptic Gregorian calendar, said so. | basics.md clock.weekday | reasonable default |
| Weeks start on Monday in both languages. | basics.md conventions | reasonable default |

## Calc

| Decision | Where | Class |
|---|---|---|
| Exact decimals and fractions; integers exact up to 30 digits, else 10 significant digits or scientific notation with `≈`; refused beyond 10^1000 or 100-digit inputs. | basics.md calc.arithmetic | reasonable default |
| Unary minus binds looser than a power; powers associate right; implicit multiplication is asked, not guessed. | basics.md calc.arithmetic | reasonable default |
| Percentages: five shapes, two decimals, `40 %` in Spanish; no VAT rate assumed. | basics.md calc.percent | reasonable default |
| Units: fixed categories; US gallons; decimal data prefixes by default; years in days by average length; bare "degrees" is the non-target unit; no currency. | basics.md calc.convert | reasonable default |
| With remembered units and no target, the target is the counterpart in that system (cross-review: stated in state.md, missing from basics.md). | basics.md calc.convert; state.md | reasonable default |

## Random and fortune

| Decision | Where | Class |
|---|---|---|
| Dice: 1–100 dice, 2–1000 sides, one modifier; faces listed up to ten dice. Coins up to 100. Numbers up to 10^15, inclusive bounds, reversed bounds reordered. | basics.md random | reasonable default |
| A pick needs a verb of choosing; repeated options count once; "elige entre 1 y 3" picks between two options. Bare "¿pizza o sushi?" is the reflector's S16 (cross-review: basics.md had `unhandled`, reflector.md sent it to `random.pick`). | basics.md random.pick; reflector.md 2.1, S8 | reasonable default |
| A fortune never repeats the previous entry; it never predicts or decides ("dime sí o no" with a verb of choosing is `random.pick`; "¿debería…?" is S27). | basics.md fortune.tell | reasonable default |
| "Otro"/"another one" means another fortune only right after a fortune; elsewhere (including as a first message) it is the catch-all, `unhandled` (cross-review: basics.md had `deflected`, which no reflector shape produces). It does not repeat dice or coins. | basics.md fortune.tell | reasonable default |

## Smalltalk, identity, help

| Decision | Where | Class |
|---|---|---|
| A greeting or thanks prefix on a request is dropped; "buenas noches" is always a farewell; "gracias, adiós" is a farewell. | basics.md conventions, smalltalk | reasonable default |
| "How are you" is answered with a factual status line, never a mood. | basics.md smalltalk.how_are_you | reasonable default |
| `identity.who` vs `identity.nature`: name and introduction ("quién eres", "qué eres", "tell me about yourself") are `who`; what kind of thing, feelings, consciousness, understanding, named assistants are `nature`; questions presupposing a life ("¿sueñas?", "¿te aburres?") are the reflector's S17 (cross-review: basics.md had "¿sueñas?" as `nature`). | basics.md identity; reflector.md S15, S17 | reasonable default |
| Comparisons with other assistants are `unhandled · identity.nature`. | basics.md identity.nature | reasonable default |
| Origin questions. | basics.md identity.who | see ruling 4 |
| `help.capabilities` lists enabled skills' summaries in catalogue order, user skills alphabetically; `help.skill` gives the summary and two or three example phrases and says when a skill is disabled. No conflict between the files was found. | basics.md help | resolved 2026-11-24 by amending the design; was a conflict: SKILLS.md §1 has no manifest field for a skill's everyday names ("reloj", "calculadora") or for help examples distinct from classifier training data |

## Preferences

| Decision | Where | Class |
|---|---|---|
| Only name, place and units; anything else declined. | state.md preferences | reasonable default |
| A name is stored as typed with a leading capital; an unknown place is not stored. | state.md preferences.set | reasonable default |
| `teach.assert` vs `preferences.set`: statements about the user ("me llamo", "vivo en", "recuerda que vivo en…") are preferences; world facts are `teach.assert` only with a teaching cue, otherwise the reflector (cross-review: state.md sent bare world facts to `teach.assert`). | state.md; knowledge.md teach | reasonable default (KNOWLEDGE.md §6) |
| `teach.forget` vs `preferences.forget`: "forget" + name/place/units/"me"/"everything about me" is preferences; + a taught fact, topic, phrase or "everything I taught you" is teach; bare "olvídalo"/"forget it" is a cancellation or an acknowledgement; bare "olvida"/"forget" asks which preference. | state.md; knowledge.md teach.forget; conversation.md 3.5 | reasonable default |
| Forgetting all preferences asks first; a named key is forgotten at once. | state.md preferences.forget | reasonable default |
| A write recognised only by the classifier is confirmed first (cross-review: the specs did not apply DESIGN.md §11). | state.md; knowledge.md teach | reasonable default (DESIGN.md §11, ADR 0008) |

## Knowledge: lookup, senses, reasoning

| Decision | Where | Class |
|---|---|---|
| `define.word` vs `relations.about`: "qué es X"/"what is X" is `define.word`; "qué sabes de X"/"tell me about X" is `relations.about`; a definitional question never reaches the reflector (cross-review: reflector.md 4.1 claimed "¿qué es el amor?"). | knowledge.md; reflector.md 4.1 | reasonable default |
| Longest known word wins; conversation language first, then the other; near misses (one edit, one candidate, conversation language first) asked as yes/no. | knowledge.md 2.1, decisions 10, 12 | reasonable default |
| Sense order: remembered, turn context, previous three turns, structural fit, strict rank, then ask; the choice is remembered for the conversation, never on disk. | knowledge.md 2.2, decisions 7–8 | reasonable default |
| Three kinds of "no"; "beyond depth" only when the search is cut from both ends (cross-review: as first written, "is a drum a string instrument?" would have been "beyond depth"). | knowledge.md 2.4, decision 2 | reasonable default |
| Confidence bands and numbers. | knowledge.md 1.5, 2.5 | see ruling 3 |
| Paths in full up to three links, else first, last and count. | knowledge.md 2.3, decision 5 | reasonable default |
| Taught knowledge is always attributed; pack knowledge only when asked or in conflict. | knowledge.md decision 6 | reasonable default |
| Ambiguous Spanish causal questions are answered in both directions. | knowledge.md decision 13 | reasonable default |
| List caps: kinds/parts 15 with "all", about five items, teach.list ten. | knowledge.md decision 14 | reasonable default |
| Missing gloss falls back to relations, other language's gloss only on request. | knowledge.md decision 11 | see ruling 8 |
| Translation lists one translation per sense instead of asking. | knowledge.md decision 9 | resolved 2026-11-24 by amending the design; was a conflict: KNOWLEDGE.md §5 step 4 says Talos asks when no sense wins |

## Teaching

| Decision | Where | Class |
|---|---|---|
| Closed lists of cues and shapes; "recuerda que" teaches only about things other than the user; opinions, quantities and dated facts are unsupported; taught claims carry no validity interval. | knowledge.md teach, decisions 15, 22 | reasonable default |
| Ordinary statements are never stored, never used as context, and Talos gives no hint they could be taught. | knowledge.md decision 16 | reasonable default (KNOWLEDGE.md §6, §8) |
| Unknown subject becomes a new word without asking; unknown object is asked; new words stored as typed. | knowledge.md decision 17 | reasonable default |
| Re-teaching known facts stores nothing. | knowledge.md decision 18 | reasonable default |
| Contradictions: pack → ask to override; user → ask to replace; derived → ask which link; an `is_a` cycle is refused without asking; a subclass exception is stored without asking. | knowledge.md decision 19 | resolved 2026-11-24 by amending the design; was a conflict: KNOWLEDGE.md §4 says Talos asks on a cycle too |
| Forgetting: user claims at once, pack claims with confirmation and a tombstone; topic and "all" cover taught knowledge only; re-teaching lifts a tombstone. | knowledge.md decision 20 | reasonable default |
| Taught phrases match the whole normalised message, in either language, never map to deleting intents or `teach.phrase`, and ask before shadowing a meaning. | knowledge.md decision 21 | reasonable default |

## Translation and the word-pair lexicon

| Decision | Where | Class |
|---|---|---|
| Free sentences are refused even when every word is known; no third languages; inflected forms kept when the target has them. | knowledge.md translate, decision 23 | reasonable default (ADR 0012) |
| The lexicon (KNOWLEDGE.md §7 role 1) is consulted only after the concept store, always hedged, never used by `define`/`relations`, never reasoned from; roles 2 and 3 stay out until measured. | knowledge.md translate.word, decision 24 | see ruling 9 |

## Reflector

| Decision | Where | Class |
|---|---|---|
| 29 shapes, ranked: distress, requests Talos cannot meet, remarks at Talos, conversational acts, statements about the user's world. | reflector.md 2.1, decision 1 | reasonable default |
| The substitution test defines "never imply understanding". | reflector.md 1, decision 4 | reasonable default |
| At most one hand-back question, never about feelings or obstacles. | reflector.md decision 5 | reasonable default |
| Unpleasant self-states get acknowledgement only. | reflector.md S5, decision 6 | reasonable default |
| S26 ("you don't understand") invites a rephrasing, the one allowed invitation. | reflector.md S26 | reasonable default; an exception to README's "no unsolicited suggestions" |
| Disagreement after a stored answer says it is what Talos has stored, without pointing to `teach`. | reflector.md decision 15 | reasonable default |
| One-directional reflection; no second-person or first-plural echoes; no ambiguous verb forms or enclitic groups; no gendered words; echo ≤ 12 words; never insults, long numbers, e-mails, URLs, codes or quoted speech. | reflector.md 3, decisions 8, 10–12 | reasonable default |
| Long or pasted text (over about 60 words or multi-line) is `unhandled · long-text`. | reflector.md 4.6, decision 13 | reasonable default |
| "Not yet" capabilities say "todavía no lo sé hacer". | reflector.md 4.3; conversation.md 1.2 | reasonable default (borderline with conversation.md 4.1's "no promise to do something later") |
| Metric guards: zero false deflection on generative and distress input; forbidden-phrase list fails the build; floors set after the first battery run. | reflector.md 7, decision 19 | reasonable default |
| Distress reply and its pipeline position. | reflector.md S29 | see rulings 1–2 |
| Tú only; other languages of Spain. | reflector.md 3.2, 4.5 | see rulings 12–13 |
