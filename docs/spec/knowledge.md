# Knowledge: define, relations, teach, translate

The four skills that sit on the knowledge store. They are what makes Talos a
semantic assistant rather than a command bot, so this file decides how the
store behaves towards a user: how a word is resolved to a meaning, how a
derived answer explains itself, how uncertainty and ignorance are voiced, how
Talos is taught and how it refuses to learn from anything that is not
teaching. Format and global rules are in [README.md](README.md); cross-cutting
behaviour (language, slot types, expectations, cancelling, refusals) is in
[conversation.md](conversation.md) and is not repeated here. The model behind
it is in [KNOWLEDGE.md](../KNOWLEDGE.md) and ADRs
[0005](../adr/0005-sqlite-knowledge-store.md),
[0006](../adr/0006-knowledge-packs.md),
[0011](../adr/0011-bilingual-by-construction.md) and
[0012](../adr/0012-no-language-model.md).

Every example in this file is answered from the **fixture** in section 1 and
nothing else, so each exchange can be checked by hand and replayed as a golden
transcript. Unless an example says otherwise:

```text
Given: now = 2027-03-14 18:42 Europe/Madrid (a Sunday); a new conversation;
       user state U0 (nothing taught); installed packs: the fixture pack "core"
       and the fixture word-pair lexicon (1.8).
```

Reply wording is illustrative. What is normative is the content of the reply
(which facts, which sense, which path, which hedge), its language, and the
outcome line.

---

## 1. Fixture

### 1.1 Sources

| Source | Kind | Shown to the user as |
|---|---|---|
| `core` 1.0 | pack, read-only | es «el paquete *core* (vocabulario básico)», "lo que traigo de serie"; en "the *core* pack (core vocabulary)", "what I came with" |
| `user` | `user.db`, read-write | es «tú», «me lo enseñaste tú»; en "you", "you taught me" |
| `lexicon` | word-pair pack, read-only (1.8) | es «un diccionario de pares de palabras, menos fiable que lo demás»; en "a word-pair dictionary, less reliable than the rest" |

`core` stands in for the real core-vocabulary pack (KNOWLEDGE.md §7). Its
claims have confidence **1.0** unless the claims table says otherwise.

### 1.2 Predicates

| Predicate | Transitive | Inverse (answered from the same claims) |
|---|---|---|
| `is_a` | yes | "kinds of" |
| `part_of` | yes | `has_part` |
| `causes` | yes | "caused by" |

Search depth limit: **6** links for every transitive predicate.

### 1.3 Concepts, words and glosses

Ranks are the baseline order of a word's senses (0 first). Every sense has a
gloss in its language unless the table says **no gloss**. Glosses quoted in
examples are written out; the others exist but are not needed.

| Concept | es lexemes (rank) — gloss | en lexemes (rank) — gloss |
|---|---|---|
| `core:entity` | entidad | entity |
| `core:physical_entity` | entidad física | physical entity |
| `core:object` | objeto | object |
| `core:artifact` | artefacto | artifact, artefact |
| `core:device` | dispositivo, aparato | device |
| `core:musical_instrument` | instrumento musical, instrumento — «objeto que sirve para producir música» | musical instrument, instrument — "a device for making music" |
| `core:string_instrument` | instrumento de cuerda — «instrumento musical que suena al hacer vibrar sus cuerdas» | string instrument, stringed instrument — "an instrument that sounds when its strings are made to vibrate" |
| `core:wind_instrument` | instrumento de viento | wind instrument |
| `core:percussion_instrument` | instrumento de percusión | percussion instrument |
| `core:electronic_instrument` | instrumento electrónico | electronic instrument |
| `core:guitar` | guitarra — «instrumento de cuerda con mástil y caja de resonancia, que se toca punteando o rasgueando las cuerdas» | guitar — "a string instrument with a fretted neck and a hollow body, played by plucking or strumming" |
| `core:electric_guitar` | guitarra eléctrica | electric guitar |
| `core:classical_guitar` | guitarra clásica, guitarra española | classical guitar, Spanish guitar |
| `core:violin` | violín — «instrumento de cuerda que se toca con arco, apoyado entre el hombro y la barbilla» | violin, fiddle — "a string instrument played with a bow and held under the chin" |
| `core:bandurria` | bandurria — «instrumento de cuerda pequeño, con seis pares de cuerdas, que se toca con púa» | *(no English lexeme)* |
| `core:dulcimer` | *(no Spanish lexeme)* | dulcimer — "a string instrument whose strings are stretched over a flat sound box" |
| `core:flute` | flauta | flute |
| `core:drum` | tambor | drum |
| `core:synthesizer` | sintetizador — «instrumento electrónico que genera y modifica sonidos» | synthesizer, synthesiser — "an electronic instrument that generates and shapes sounds" |
| `core:neck` | mástil | neck (1) — "the long narrow part of a string instrument, where the strings are pressed" |
| `core:neck_body` | cuello | neck (0) — "the part of the body that joins the head to the shoulders" |
| `core:sound_box` | caja de resonancia — «cuerpo hueco del instrumento que amplifica el sonido» | sound box, soundbox — "the hollow body of an instrument that makes its sound louder" |
| `core:string` | cuerda (1) — «hilo tenso de un instrumento que suena al vibrar» | string — "a stretched cord on an instrument that sounds when it vibrates" |
| `core:rope` | cuerda (0), soga — «conjunto de hilos retorcidos que sirve para atar o colgar cosas» | rope |
| `core:fret` | traste — **no gloss** | fret — "one of the raised metal strips across the neck of a guitar" |
| `core:capo` | cejilla — «pieza que se sujeta al mástil para subir a la vez el tono de todas las cuerdas» | capo — **no gloss** |
| `core:pickup` | pastilla (1) — «pieza de la guitarra eléctrica que convierte la vibración de las cuerdas en señal eléctrica» | pickup — "the part of an electric guitar that turns string vibration into an electric signal" |
| `core:pill` | pastilla (0), píldora — «porción pequeña y sólida de medicamento que se traga» | pill, tablet |
| `core:bank` | banco (0) — «empresa que guarda dinero, da préstamos y ofrece otros servicios financieros» | bank (0) — "a business that keeps money, lends it and offers other financial services" |
| `core:financial_institution` | entidad financiera | financial institution |
| `core:organisation` | organización | organization, organisation |
| `core:company` | empresa | company |
| `core:bench` | banco (0) — «asiento largo en el que caben varias personas» | bench — "a long seat for several people" |
| `core:seat` | asiento | seat |
| `core:furniture` | mueble | piece of furniture, furniture |
| `core:school_of_fish` | banco (2), banco de peces — «grupo grande de peces que nadan juntos» | school (1), shoal — "a large group of fish swimming together" |
| `core:school` | escuela, colegio | school (0) — "a place where children are taught" |
| `core:group` | grupo | group |
| `core:riverbank` | orilla, ribera | bank (1), riverbank — "the land along the side of a river" |
| `core:river` | río | river |
| `core:heat` | calor | heat |
| `core:evaporation` | evaporación | evaporation |
| `core:cloud` | nube | cloud |
| `core:rain` | lluvia, llover (verb) | rain (noun and verb) |
| `core:flood` | inundación | flood |
| `core:storm` | tormenta | storm |
| `core:drought` | sequía | drought |
| `core:earthquake` | terremoto | earthquake |
| `core:cold` | frío | cold (0) — "a low temperature", cold weather |
| `core:common_cold` | resfriado, catarro — «infección leve de nariz y garganta» | cold (0) — "a mild infection of the nose and throat", common cold |
| `core:virus` | virus | virus |
| `core:whale` | ballena | whale |
| `core:mammal` | mamífero | mammal |
| `core:fish` | pez | fish |
| `core:animal` | animal | animal |
| `core:organism` | organismo, ser vivo | organism, living thing |
| `core:celestial_body` | cuerpo celeste | celestial body, heavenly body |
| `core:planet` | planeta | planet |
| `core:dwarf_planet` | planeta enano | dwarf planet |
| `core:pluto` | Plutón | Pluto |
| `core:earth` | Tierra | Earth |
| `core:mars` | Marte | Mars |
| `core:solar_system` | sistema solar | solar system |

Forms: every noun has its plural ("guitarras", "violines", "cuerdas",
"bancos", "guitars", "violins", "drums", "fish"…); "llover" has its
conjugated forms ("llueve", "llovía"…), "rain" its verb forms ("rains",
"raining").

Words that appear in examples and have no lexeme in **either** language:
*zanfona*, *hurdy-gurdy*, *theremin*, *teremín*, *clavijero*, *guitarrón*,
*castañuelas*, *sinte*, *synth*, *tuning peg*, *zorongo*, *glarb*, *física
cuántica*, and the lexicon-only words of 1.8.

### 1.4 Claims

| # | Claim | Conf. | Valid |
|---|---|---|---|
| C01 | guitar `is_a` string_instrument | | |
| C02 | electric_guitar `is_a` guitar | | |
| C03 | classical_guitar `is_a` guitar | | |
| C04 | violin `is_a` string_instrument | | |
| C05 | bandurria `is_a` string_instrument | | |
| C06 | dulcimer `is_a` string_instrument | | |
| C07 | string_instrument `is_a` musical_instrument | | |
| C08 | wind_instrument `is_a` musical_instrument | | |
| C09 | percussion_instrument `is_a` musical_instrument | | |
| C10 | electronic_instrument `is_a` musical_instrument | | |
| C11 | flute `is_a` wind_instrument | | |
| C12 | drum `is_a` percussion_instrument | | |
| C13 | synthesizer `is_a` electronic_instrument | | |
| C14 | musical_instrument `is_a` device | | |
| C15 | device `is_a` artifact | | |
| C16 | artifact `is_a` object | | |
| C17 | object `is_a` physical_entity | | |
| C18 | physical_entity `is_a` entity | | |
| C19 | neck `part_of` guitar | | |
| C20 | sound_box `part_of` guitar | | |
| C21 | string `part_of` guitar | | |
| C22 | fret `part_of` neck | | |
| C23 | pickup `part_of` electric_guitar | | |
| C24 | **not** sound_box `part_of` electric_guitar | 0.9 | |
| C25 | string `part_of` violin | | |
| C26 | sound_box `part_of` violin | | |
| C27 | bank `is_a` financial_institution | | |
| C28 | financial_institution `is_a` organisation | | |
| C29 | bench `is_a` seat | | |
| C30 | seat `is_a` furniture | | |
| C31 | school_of_fish `is_a` group | | |
| C32 | riverbank `part_of` river | | |
| C33 | heat `causes` evaporation | | |
| C34 | evaporation `causes` cloud | 0.9 | |
| C35 | cloud `causes` rain | 0.8 | |
| C36 | rain `causes` flood | 0.5 | |
| C37 | storm `causes` rain | 0.95 | |
| C38 | virus `causes` common_cold | | |
| C39 | **not** cold `causes` common_cold | 0.9 | |
| C40 | whale `is_a` mammal | | |
| C41 | **not** whale `is_a` fish | | |
| C42 | mammal `is_a` animal | | |
| C43 | fish `is_a` animal | | |
| C44 | animal `is_a` organism | | |
| C45 | organism `is_a` physical_entity | | |
| C46 | planet `is_a` celestial_body | | |
| C47 | dwarf_planet `is_a` celestial_body | | |
| C48 | pluto `is_a` planet | | until 2006-08-24 |
| C49 | pluto `is_a` dwarf_planet | | from 2006-08-24 |
| C50 | earth `is_a` planet | | |
| C51 | mars `is_a` planet | | |
| C52 | earth `part_of` solar_system | | |
| C53 | mars `part_of` solar_system | | |
| C54 | pluto `part_of` solar_system | | |
| C55 | celestial_body `is_a` physical_entity | | |

Two long chains matter for the depth limit: guitar → string instrument →
musical instrument → device → artifact → object → physical entity is **6**
links (within the limit); one more link to *entity* is 7 (beyond it).

### 1.5 Confidence parameters

| Parameter | Value |
|---|---|
| Confidence of a taught claim | 0.95 |
| Decay per link after the first, on a derived path | × 0.99 |
| Wording band **plain** | ≥ 0.85 |
| Wording band **hedged** | 0.60 – 0.85 |
| Wording band **weak** | 0.30 – 0.60 |
| Below 0.30 | not affirmed (see 2.5) |

Worked values used in examples: violin → musical instrument 0.99 (plain);
heat → rain 1.0 × 0.9 × 0.8 × 0.99² ≈ 0.71 (hedged); cloud → rain 0.8
(hedged); rain → flood 0.5 (weak); heat → flood ≈ 0.35 (weak); storm → flood
≈ 0.47 (weak); zanfona → musical instrument (taught) 0.95 × 0.99 ≈ 0.94
(plain).

### 1.6 Stock phrases

| es | en | Note |
|---|---|---|
| buenos días | good morning | |
| buenas tardes | good afternoon; good evening (until dark) | |
| buenas noches | good evening (greeting after dark); good night (leaving) | |
| de nada | you're welcome | |
| lo siento | I'm sorry | |
| encantado / encantada (de conocerte) | nice to meet you | Spanish agrees with the speaker |
| ¿qué tal? | how are you?; how's it going? | |
| ¡salud! (al estornudar) | bless you | |
| ¡salud! (al brindar) | cheers | |
| buen provecho | enjoy your meal | approximate: no fixed English equivalent |

### 1.7 Taught states

Examples that depend on teaching name one of these states in their `Given:`
line. Each item is what the user typed and when.

| State | Contents |
|---|---|
| U0 | nothing taught |
| U1 | 2027-03-10, es: «aprende: la zanfona es un instrumento de cuerda» — creates `user:zanfona` (es lexeme *zanfona*, no gloss) · 2027-03-11, en: "teach: a theremin is an electronic instrument" — creates `user:theremin` (en lexeme *theremin*, no gloss) · 2027-03-12, es: «aprende: el calor provoca sequía» |
| U2 | U1 · 2027-03-13, es: «aprende: el frío provoca resfriados», confirmed against C39 — the user claim overrides C39 |
| U3 | U1 · 2027-03-13, es: «cuando diga «tira» quiero decir «tira un dado»» · 2027-03-13, en: "when I say 'flip it' I mean 'toss a coin'" |

### 1.8 Word-pair lexicon

The fixture's stand-in for the word-pair lexicon of KNOWLEDGE.md §7: word →
candidate translations, best first, with no concepts, senses or relations.
Only these entries exist; the words marked *lexicon-only* have no lexeme in
the concept store.

| Direction | Word | Candidates, best first |
|---|---|---|
| es → en | pandereta *(lexicon-only)* | tambourine |
| es → en | gaita *(lexicon-only)* | bagpipes, bagpipe |
| es → en | tamboril *(lexicon-only)* | drum, tabor |
| es → en | guitarra | guitar |
| en → es | tambourine *(lexicon-only)* | pandereta |
| en → es | accordion *(lexicon-only)* | acordeón |
| en → es | cord *(lexicon-only)* | cuerda, cordón |
| en → es | guitar | guitarra |

"guitarra" and "guitar" are there to show that the concept store answers
first: for them the lexicon is never consulted.

---

## 2. Behaviour shared by the four skills

### 2.1 From the words typed to a concept

1. **Forms resolve to their lemma.** "guitarras" is *guitarra*, "violins" is
   *violin*, "llueve" is *llover*. The reply uses the form that reads
   naturally, not necessarily the one typed.
2. **The longest known word wins.** "caja de resonancia", "guitarra
   eléctrica", "instrumento de cuerda", "banco de peces" are single lexemes and
   are never split.
3. **Conversation language first, then the other.** A word is looked up among
   the lexemes of the conversation's language; if it is not there, among the
   other language's. A word found only in the other language is used, and the
   reply says which language the word is in when that matters ("*zanfona* is
   a Spanish word").
4. **A near miss is asked, not guessed.** If a word is unknown in both
   languages and exactly one known lemma is one letter away (one insertion,
   deletion, substitution or swap), Talos asks whether that is the word meant
   (a yes/no expectation). Candidates are looked for in the conversation's
   language first and only then in the other, as in step 3, so «guitara» in
   Spanish finds only *guitarra* although *guitar* is also one letter away.
   With zero or several candidates it does not suggest anything.
5. **An unknown word is said to be unknown.** "No conozco la palabra
   «zorongo»." / "I don't know the word 'glarb'." Status `unhandled`, intent
   kept.

### 2.2 Choosing a sense

A word with several senses goes through these steps, and the first that
decides, decides:

1. **The sense remembered in this conversation**, if the user chose one (step
   6) or one was chosen by context earlier in the conversation.
2. **Explicit context in the turn**: words of the turn that occur in a sense's
   gloss or name a concept within two links of it ("banco … dinero",
   "cuerda … guitarra", "bank … river").
3. **Recent context**: concepts discussed in the previous three turns of the
   conversation, scored the same way.
4. **Structural fit**: if the question asks for a relation (is it a kind of
   furniture? what is it part of?) and only one sense has any claim that can
   answer it, that sense wins.
5. **Baseline rank**: if one sense has a strictly lower rank than every
   other, it wins.
6. **Ask.** Otherwise (a tie at the top) Talos asks which sense is meant, as a
   `choice`. Each option is named by its nearest `is_a` class, or by a few
   words of its gloss if it has none. The answer may be the class word, a word
   from the gloss ("el de sentarse"), a position, or a number.

When a sense wins at step 5 the reply **mentions in one clause that the word
has other senses** and names them. When it wins at steps 2–4 the reply names
the sense it took in a short parenthesis so a wrong choice is visible. A
chosen sense (step 2 or 6) is **remembered for the rest of the conversation**
and applies to every skill in this file; new explicit context in a later turn
replaces it. Conversations do not share memories, and nothing about senses is
written to disk.

### 2.3 Derived answers carry their path

A direct answer (one stored claim) is stated plainly. A **derived** answer
(two or more claims, or an inherited one) always states the path that
produced it:

- paths of up to three links are given in full: *"Sí: el violín es un
  instrumento de cuerda, y los instrumentos de cuerda son instrumentos
  musicales."*
- longer paths give the first and the last link and the number of steps:
  *"Yes, in four steps: an electric guitar is a guitar, …, and a musical
  instrument is a device."* The full path is given when the user asks how
  Talos knows (`relations.provenance`).
- an inherited answer names the class it was inherited from: *"Sí: como toda
  guitarra, tiene mástil."*

### 2.4 Three kinds of "no"

A negative or empty answer is always one of these, and the reply makes clear
which:

| Kind | When | Reply says | Status |
|---|---|---|---|
| **Contradicted** | a negated claim (direct, inherited or more specific) applies | "No", and gives the contradicting claim | `answered` |
| **Nothing known** | the search finished within the depth limit and found nothing either way | "nothing I know says so", plus the nearest true fact when there is one | `unhandled`, intent kept |
| **Beyond depth** | no path within six links, and the search was cut by the depth limit with links still unexplored from **both** ends (upward from the subject and downward from the class) | "I can't find it within six steps; the link may be further" | `unhandled`, intent kept |

Talos never turns "nothing known" into "no". Not knowing that a drum is a
string instrument is not knowing that it is not one.

If either end is exhausted within the limit, the answer is "nothing known":
«¿la guitarra es un animal?» climbs six links from *guitarra* without reaching
the top, but everything below *animal* is reached in two, so no longer path
can exist.

### 2.5 Voicing confidence

Confidence is voiced in words, never as a number unless the user asks for
one (`relations.provenance`).

| Band | es | en |
|---|---|---|
| plain | no qualifier | no qualifier |
| hedged | «Por lo que sé, …» / «Diría que sí: …» | "As far as I know, …" / "I'd say so: …" |
| weak | «Puede que sí, pero no lo sé con certeza: …» | "Possibly, but I'm not sure: …" |
| below 0.30 | «Solo tengo una relación muy débil entre … y …; no lo afirmaría.» | "I only have a very weak link between … and …; I wouldn't say so." (status `unhandled`) |

Spanish wording never uses an adjective about Talos («no estoy seguro»):
Talos has no gender ([reflector.md](reflector.md) 3.2).

The band applies to the whole answer: a derived answer takes the band of its
path's confidence. Answers that rest on something **the user taught** always
say so («según me enseñaste», "you taught me"), whatever the band, because the
user should be able to tell what Talos knows from what it was told.

### 2.6 Time

Claims with a validity interval are filtered against `now` for
present-tense questions. A past-tense question ("¿era…?", "was…?") or an
explicit date ("en 1990", "in 1990") moves the reference time. An answer
that depends on an interval states the boundary date ("hasta agosto de
2006", "since August 2006"). Lists (kinds, parts) contain only what holds at
the reference time; `relations.about` also lists expired claims, marked as
past.

### 2.7 Words in replies

Concepts are named with the conversation language's lexeme (rank 0 if
several). A concept with no lexeme in the conversation language is named with
the other language's word, in quotes, marked once: «"dulcimer" (en inglés)»,
"«bandurria» (a Spanish word)". Talos never makes up a translation for it.

### 2.8 Follow-ups

The concept or claim of the last answer is the conversation's **topic**.
Short follow-ups use it: «¿y en inglés?», «¿y sus partes?», «¿cómo lo sabes?»,
"and in Spanish?", "what is it part of?", "how sure are you?". With no topic,
a follow-up that needs one asks what it is about.

### 2.9 Status

| Situation | Status |
|---|---|
| An answer grounded in claims or glosses, positive or contradicted | `answered` |
| An answer that rests on the absence of knowledge (unknown word, nothing known, beyond depth, no word in the other language) | `unhandled`, intent kept |
| A definition asked for and no gloss in the conversation language, even when relations are offered instead | `unhandled`, intent kept |
| A request this file says Talos does not do (free translation, a third language, an unsupported teaching shape) | `unhandled`, intent kept |
| Talos asks back (sense, confirmation, missing slot) | `asks · <slot>` |
| A teaching or forgetting request completed, or declined by the user after a question | `answered` |

---

## define

What a word means, in the conversation's language. The answer is a gloss from
the store for one sense of the word. When the store has the word but no gloss
in that language, Talos says so and answers from the word's relations instead;
it never shows the other language's gloss unasked (KNOWLEDGE.md §7). It does
not compose definitions, explain idioms that are not lexemes, or define
encyclopedic entities beyond what their glosses say.

### define.word

**Recognises** — asking what a word means or what a thing is, with the word
as the content: «¿qué es un/una X?», «¿qué son los X?», «¿qué significa X?»,
«¿qué quiere decir X?», «define X», «defíneme X», «significado de X», «¿me
dices qué es X?», «X, ¿qué es?»; "what is a X?", "what's an X?", "what are
Xs?", "what does X mean?", "define X", "meaning of X", "X — what's that?".
The word may be quoted or bare, singular or plural, with or without article.
A word that exists only in the other language is accepted (2.1).

**Slots**
- `word` — `term`, required. Asked if missing.
- `sense` — `choice`, optional; filled by 2.2, asked on a tie.

**Answers** — the gloss of the chosen sense in the conversation language,
introduced by the word. If a sense won on rank alone, one clause naming the
other senses. If the sense has no gloss in this language: a statement that
there is no definition in this language, then the word's nearest `is_a`
claim, or failing that its `part_of` claim, as a short description; and the
other language's word if there is one. If there is neither gloss nor any
claim: only the statement that there is no definition in this language, and
that there is one in the other language if so. Either way the definition
asked for was not given, so the status is `unhandled` with the intent kept
(README, "Which status an outcome gets"): a host may have one. If the other
language's gloss is explicitly asked for, it is given in quotes and labelled
with its language.

**Does not**
- «¿qué sabes de X?», "tell me about X" → `relations.about`.
- «¿cómo se dice X en inglés?», "what's X in Spanish?" → `translate.word`.
- «¿qué tipos de X hay?» → `relations.kinds`; «¿X es un Y?» → `relations.is_a`.
- «¿qué es la vida?», «¿qué es el amor?» with no such lexeme: an unknown word
  (2.1), not philosophy; never reflected.
- «¿qué es esto?», "what's that?" with nothing to refer to: asks for the word.
- «¿qué significa «buen provecho»?» (a stock phrase, not a lexeme): answered
  by `translate.word` only if a translation is asked; as a definition it is
  an unknown word.
- «¿qué eres?» / "what are you?" → `identity.who`.

#### Examples

```text
[es] U: ¿qué es una guitarra?
     T: Una guitarra es un instrumento de cuerda con mástil y caja de
        resonancia, que se toca punteando o rasgueando las cuerdas.
                                           → answered · define.word · word=guitarra · sense=core:guitar

[es] U: qué son los violines
     T: El violín es un instrumento de cuerda que se toca con arco, apoyado
        entre el hombro y la barbilla.
                                           → answered · define.word · word=violín (from "violines")

[es] U: define «caja de resonancia»
     T: La caja de resonancia es el cuerpo hueco del instrumento que amplifica
        el sonido.                         → answered · define.word · word=caja de resonancia

[es] U: ¿qué significa sintetizador?
     T: Un sintetizador es un instrumento electrónico que genera y modifica
        sonidos.                           → answered · define.word · word=sintetizador

[es] U: ¿qué es una cuerda?
     T: Una cuerda es un conjunto de hilos retorcidos que sirve para atar o
        colgar cosas. También es cada uno de los hilos de un instrumento
        musical.                           → answered · define.word · word=cuerda · sense=core:rope
     (rank decides: "rope" is rank 0, the instrument sense is mentioned)

[es] U: ¿qué partes tiene una guitarra?
     T: Mástil, caja de resonancia y cuerdas.
                                           → answered · relations.parts · whole=guitarra
[es] U: ¿y qué es una cuerda?
     T: Una cuerda (de instrumento) es un hilo tenso que suena al vibrar.
                                           → answered · define.word · word=cuerda · sense=core:string
     (recent context: the guitar of the previous turn is one link from core:string)

[es] U: ¿qué es una pastilla?
     T: Una pastilla es una porción pequeña y sólida de medicamento que se
        traga. También es una pieza de la guitarra eléctrica.
                                           → answered · define.word · word=pastilla · sense=core:pill

[es] U: ¿qué es un traste?
     T: No tengo una definición de «traste» en español. Sé que es una parte
        del mástil; en inglés se dice «fret».
                                           → unhandled · define.word · word=traste · sense=core:fret
     (no Spanish gloss: falls back to C22; the English gloss is not shown;
      no definition was given, so unhandled)
```

```text
[es] U: ¿qué es un banco?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense   (define.word)
[es] U: el de sentarse
     T: Un banco es un asiento largo en el que caben varias personas.
                                           → answered · define.word · word=banco · sense=core:bench
[es] U: ¿y un banco es un mueble?
     T: Sí: un banco (de sentarse) es un asiento, y los asientos son
        muebles.                           → answered · relations.is_a · subject=banco · class=mueble
     (the choice is remembered; it applies to relations too)
[es] U: ¿cómo se dice en inglés?
     T: «Bench».                           → answered · translate.word · term=banco · sense=core:bench
```

```text
[en] U: what's a guitar?
     T: A guitar is a string instrument with a fretted neck and a hollow body,
        played by plucking or strumming.
                                           → answered · define.word · word=guitar · sense=core:guitar

[en] U: what are fiddles?
     T: A fiddle is a violin: a string instrument played with a bow and held
        under the chin.                    → answered · define.word · word=fiddle (from "fiddles")

[en] U: what does "sound box" mean?
     T: A sound box is the hollow body of an instrument that makes its sound
        louder.                            → answered · define.word · word=sound box

[en] U: define synthesiser
     T: A synthesiser is an electronic instrument that generates and shapes
        sounds.                            → answered · define.word · word=synthesiser

[en] U: what is a school?
     T: A school is a place where children are taught. "School" can also mean
        a large group of fish.             → answered · define.word · word=school · sense=core:school

[en] U: what's a bank, as in the side of a river?
     T: A bank (of a river) is the land along the side of a river.
                                           → answered · define.word · word=bank · sense=core:riverbank
     (explicit context: "river" names a concept one link from core:riverbank)

[en] U: what's a neck?
     T: A neck is the part of the body that joins the head to the shoulders.
        It's also the long part of a string instrument where the strings are
        pressed.                           → answered · define.word · word=neck · sense=core:neck_body

[en] U: what's a fret?
     T: A fret is one of the raised metal strips across the neck of a guitar.
                                           → answered · define.word · word=fret · sense=core:fret
     (English has the gloss that Spanish lacks)
```

```text
[en] U: what does cold mean?
     T: Cold as in a low temperature, or the illness?
                                           → asks · sense   (define.word)
[en] U: the illness
     T: A cold is a mild infection of the nose and throat.
                                           → answered · define.word · word=cold · sense=core:common_cold
[en] U: what causes a cold?
     T: A virus.                           → answered · relations.causes · topic=cold · direction=causes_of
     (the remembered sense is used; no second question)
```

#### Edge cases

```text
[es] U: ¿qué es un zorongo?
     T: No conozco la palabra «zorongo».   → unhandled · define.word · word=zorongo
     (unknown in both languages, no near miss)

[en] U: what's a glarb?
     T: I don't know the word "glarb".     → unhandled · define.word · word=glarb

[es] U: ¿qué es una guitara?
     T: ¿Te refieres a «guitarra»?         → asks · word (yes/no)
[es] U: sí
     T: Una guitarra es un instrumento de cuerda con mástil y caja de
        resonancia, que se toca punteando o rasgueando las cuerdas.
                                           → answered · define.word · word=guitarra
     (one known lemma at one letter's distance; asked, never assumed)

[es] U: ¿qué significa?
     T: ¿Qué palabra?                      → asks · word
     (no word and no topic)

[en] U: what's a capo?
     T: I don't have an English definition of "capo". In Spanish it's
        «cejilla», and I only have a Spanish definition.
                                           → unhandled · define.word · word=capo
     (no English gloss and no claims to fall back on; the Spanish gloss is not shown unasked)
[en] U: give me the Spanish one then
     T: In Spanish: «pieza que se sujeta al mástil para subir a la vez el tono
        de todas las cuerdas».             → answered · define.word · word=capo · gloss_lang=es
     (asked for explicitly: given in quotes and labelled)

Given: U1.
[es] U: ¿qué es una zanfona?
     T: No tengo una definición de «zanfona». Según me enseñaste, es un
        instrumento de cuerda.             → unhandled · define.word · word=zanfona · sense=user:zanfona
     (taught concept without gloss: falls back to its is_a claim, attributed)

Given: U1.
[en] U: what's a zanfona?
     T: "Zanfona" is a Spanish word you taught me: a string instrument. I
        don't have an English word or a definition for it.
                                           → unhandled · define.word · word=zanfona
     (taught in Spanish, asked in English: found in the other language's lexemes)

Given: U1.
[es] U: ¿qué es un theremin?
     T: No tengo una definición de «theremin», que solo conozco en inglés.
        Según me enseñaste, es un instrumento electrónico.
                                           → unhandled · define.word · word=theremin · sense=user:theremin

[es] U: ¿qué es bench?
     T: «Bench» es una palabra inglesa: es un banco, un asiento largo en el
        que caben varias personas.         → answered · define.word · word=bench · sense=core:bench
     (only an English lexeme: Spanish gloss and Spanish word given)

[en] U: what's a banco?
     T: "Banco" is Spanish. Do you mean a bank, a bench, or a school of fish?
                                           → asks · sense   (define.word)
     (a Spanish word with a tie at rank 0; options named in English)

[es] U: ¿qué es un banco?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense
[es] U: déjalo
     T: Vale, lo dejo.                     → answered · no intent · expectation cancelled
     (cancelled: nothing is remembered about "banco")

[es] U: ¿qué es un banco?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense
[es] U: el segundo
     T: Un banco es un asiento largo en el que caben varias personas.
                                           → answered · define.word · sense=core:bench
[es] U: ¿y el banco que da préstamos, qué es?
     T: Un banco (entidad financiera) es una empresa que guarda dinero, da
        préstamos y ofrece otros servicios financieros.
                                           → answered · define.word · sense=core:bank
     (new explicit context, «préstamos» is in the gloss of core:bank; it replaces the remembered choice from now on)

[es] U: ¿qué es un banco donde guardo el dinero?
     T: Un banco (entidad financiera) es una empresa que guarda dinero, da
        préstamos y ofrece otros servicios financieros.
                                           → answered · define.word · sense=core:bank
     (context in the turn decides; no question asked)

[en] U: what is love?
     T: I don't know the word "love".      → unhandled · define.word · word=love
     (not in the store; Talos does not philosophise)
```

---

## relations

Questions about how concepts relate: is X a kind of Y, what kinds of X there
are, what X is made of and what it is part of, what causes what, everything
known about X, and how sure Talos is of an answer and where it came from.
Answers come only from claims in the store and the bounded reasoning in
KNOWLEDGE.md §4: transitivity up to six links, inheritance with more specific
exceptions, inverse readings, validity in time. Every derived answer states
its path (2.3); every negative says which kind of "no" it is (2.4). It does
not answer comparisons ("¿qué es más grande…?"), quantities, properties that
are not relations ("¿de qué color es…?"), or "why" questions that are not
about causes.

### relations.is_a

**Recognises** — whether something is a kind or an instance of something
else: «¿X es un Y?», «¿es X un Y?», «¿los X son Y?», «¿X es un tipo / una
clase de Y?», «¿X cuenta como Y?», «¿X era un Y (en 1990)?», also without
the opening «¿»: «las ballenas son peces?». English: "is X a Y?", "are Xs
Ys?", "is X a kind / type of Y?", "does X count as a Y?", "was X a Y (in
1990)?". A sentence without any question mark and without question word
order is a statement, not this intent (see `teach.assert`, Does not).

**Slots**
- `subject` — `term`, required.
- `class` — `term`, required.
- `time` — `datetime`, optional; default `now`; set by a past tense to "before
  `now`, at the latest time a claim held".

**Answers** — «Sí» / "Yes" with the path if derived; «No» / "No" with the
contradicting claim; or one of the two unhandled negatives of 2.4, with the
subject's nearest known class when there is one. Time-bounded answers state
the boundary date. Confidence wording by 2.5.

**Does not**
- «¿qué es X?» → `define.word`; «¿qué tipos de X hay?» → `relations.kinds`.
- «¿X es parte de Y?» → `relations.part_of`.
- «¿X es mejor que Y?», «¿es grande una ballena?» (comparisons, adjectives):
  `unhandled`, no intent.
- «la guitarra es un instrumento de cuerda» with no question mark: a
  statement; not this intent and not learnt.

#### Examples

```text
[es] U: ¿la guitarra es un instrumento de cuerda?
     T: Sí.                                → answered · relations.is_a · subject=guitarra · class=instrumento de cuerda
     (direct: C01)

[es] U: ¿el violín es un instrumento musical?
     T: Sí: el violín es un instrumento de cuerda, y los instrumentos de
        cuerda son instrumentos musicales. → answered · relations.is_a · subject=violín · class=instrumento musical
     (C04 + C07)

[es] U: ¿las ballenas son peces?
     T: No: la ballena es un mamífero, no un pez.
                                           → answered · relations.is_a · subject=ballena · class=pez
     (contradicted: C41; the positive C40 is given as the nearest fact)

[es] U: ¿Plutón es un planeta?
     T: Ya no: se consideró planeta hasta agosto de 2006. Desde entonces es un
        planeta enano.                     → answered · relations.is_a · subject=Plutón · class=planeta
     (C48 expired at now; C49 holds)

[es] U: ¿Plutón era un planeta en 1990?
     T: Sí: hasta agosto de 2006 se consideraba planeta.
                                           → answered · relations.is_a · subject=Plutón · class=planeta · time=1990

[es] U: ¿una guitarra eléctrica es un aparato?
     T: Sí, en cuatro pasos: la guitarra eléctrica es una guitarra, …, y un
        instrumento musical es un aparato. → answered · relations.is_a · subject=guitarra eléctrica · class=aparato
     (C02, C01, C07, C14; more than three links: first, last and the count)

[es] U: ¿la guitarra es un animal?
     T: Nada de lo que sé dice que la guitarra sea un animal. Sé que es un
        instrumento de cuerda.             → unhandled · relations.is_a · subject=guitarra · class=animal
     (nothing known; not turned into "no")

Given: U1.
[es] U: ¿la zanfona es un instrumento musical?
     T: Sí: según me enseñaste, la zanfona es un instrumento de cuerda, y los
        instrumentos de cuerda son instrumentos musicales.
                                           → answered · relations.is_a · subject=zanfona · class=instrumento musical
     (taught claim + C07; 0.94, plain, attributed)
```

```text
[en] U: is a guitar a string instrument?
     T: Yes.                               → answered · relations.is_a · subject=guitar · class=string instrument

[en] U: are violins musical instruments?
     T: Yes: a violin is a string instrument, and string instruments are
        musical instruments.               → answered · relations.is_a · subject=violin · class=musical instrument

[en] U: is a whale a fish?
     T: No: a whale is a mammal, not a fish.
                                           → answered · relations.is_a · subject=whale · class=fish

[en] U: is Pluto a planet?
     T: Not any more: it was counted as a planet until August 2006. Since then
        it's a dwarf planet.               → answered · relations.is_a · subject=Pluto · class=planet

[en] U: was Pluto a planet in 1990?
     T: Yes: until August 2006 it was counted as a planet.
                                           → answered · relations.is_a · subject=Pluto · class=planet · time=1990

[en] U: is Pluto a celestial body?
     T: Yes: Pluto is a dwarf planet, and dwarf planets are celestial bodies.
                                           → answered · relations.is_a · subject=Pluto · class=celestial body
     (through C49, the claim valid now; the expired C48 is not used)

[en] U: is a drum a string instrument?
     T: Nothing I know says a drum is a string instrument. I know it's a
        percussion instrument.             → unhandled · relations.is_a · subject=drum · class=string instrument

Given: U1.
[en] U: is a theremin a musical instrument?
     T: Yes: you taught me a theremin is an electronic instrument, and
        electronic instruments are musical instruments.
                                           → answered · relations.is_a · subject=theremin · class=musical instrument
```

#### Edge cases

```text
[es] U: ¿la guitarra es una entidad?
     T: No lo encuentro a menos de seis pasos: la relación, si existe, está
        más lejos.                         → unhandled · relations.is_a · subject=guitarra · class=entidad
     (seven links needed; beyond depth, distinct from "nothing known")

[en] U: is a guitar a physical entity?
     T: Yes, in six steps: a guitar is a string instrument, …, and an object
        is a physical entity.              → answered · relations.is_a · subject=guitar · class=physical entity
     (exactly six links: within the limit)

[es] U: ¿un banco es una organización?
     T: Sí: un banco (entidad financiera) es una entidad financiera, y las
        entidades financieras son organizaciones.
                                           → answered · relations.is_a · subject=banco · class=organización
     (tie on rank, but only core:bank reaches "organización": structural fit, sense named)

[es] U: las ballenas son peces?
     T: No: la ballena es un mamífero, no un pez.
                                           → answered · relations.is_a
     (no opening «¿», as many people type: the closing «?» makes it a question)

[es] U: las ballenas son peces
     T: (respuesta del reflector, p. ej. «¿Y eso?»)
                                           → deflected · no intent · nothing stored
     (no question mark: a statement; not answered, not corrected, not learnt)

[en] U: is a guitar?
     T: Is a guitar what?                  → asks · class   (relations.is_a)

[es] U: ¿la flauta es un instrumento de cuerda o de viento?
     T: De viento: la flauta es un instrumento de viento.
                                           → answered · relations.is_a · subject=flauta · class=instrumento de viento
     (a two-option question: the option with a claim is given; the other is not called false)

Given: U2.
[es] U: ¿el frío es un virus?
     T: Nada de lo que sé dice que el frío sea un virus.
                                           → unhandled · relations.is_a · subject=frío · class=virus
     (taught claims of another predicate do not leak into is_a)
```

### relations.kinds

**Recognises** — the kinds, types, subclasses or known instances of
something: «¿qué tipos de X hay?», «¿qué clases de X conoces?», «¿qué X
conoces?», «dime X» (with a plural class), «dime tipos de X», «dime todos los
X que conozcas»; "what kinds of X are there?", "what types of X do you
know?", "which Xs do you know?", "name some Xs", "list all the Xs you know".

**Slots**
- `class` — `term`, required.
- `depth` — `choice` (`direct` | `all`), optional; default `direct`; `all`
  when the user says «todos», «todas», "all", "every".
- `time` — `datetime`, optional; default `now`.

**Answers** — the direct subclasses and instances of the class (reverse
`is_a`, one level), valid at the reference time, as a short list in the
conversation language, taught ones attributed. With `all`: the closure to the
depth limit, grouped under their direct parent, at most 15 items, then the
count of the rest. A class with nothing below it is "nothing known"
(`unhandled`).

**Does not**
- «¿qué partes tiene X?» → `relations.parts`.
- «¿qué es X?» → `define.word`.
- «¿qué instrumento me recomiendas?» (a recommendation): `unhandled`.

#### Examples

```text
[es] U: ¿qué tipos de instrumentos musicales hay?
     T: Conozco cuatro: de cuerda, de viento, de percusión y electrónicos.
                                           → answered · relations.kinds · class=instrumento musical

[es] U: ¿qué instrumentos de cuerda conoces?
     T: La guitarra, el violín, la bandurria y el "dulcimer" (en inglés; no
        sé cómo se dice en español).       → answered · relations.kinds · class=instrumento de cuerda
     (C01, C04, C05, C06; core:dulcimer has no Spanish lexeme)

[es] U: ¿qué tipos de guitarra hay?
     T: La guitarra eléctrica y la clásica.
                                           → answered · relations.kinds · class=guitarra

[es] U: dime planetas
     T: La Tierra y Marte.                 → answered · relations.kinds · class=planeta
     (Plutón is not listed: C48 does not hold now)

[es] U: ¿qué planetas había en el año 2000?
     T: La Tierra, Marte y Plutón.         → answered · relations.kinds · class=planeta · time=2000

[es] U: ¿qué clases de animales conoces?
     T: Mamíferos y peces.                 → answered · relations.kinds · class=animal

[es] U: dime todos los instrumentos que conozcas
     T: De cuerda: guitarra (eléctrica y clásica), violín, bandurria y
        "dulcimer". De viento: flauta. De percusión: tambor. Electrónicos:
        sintetizador.                      → answered · relations.kinds · class=instrumento musical · depth=all

Given: U1.
[es] U: ¿qué instrumentos electrónicos conoces?
     T: El sintetizador y el "theremin" (este me lo enseñaste tú, en inglés).
                                           → answered · relations.kinds · class=instrumento electrónico
```

```text
[en] U: what kinds of musical instruments are there?
     T: I know four: string, wind, percussion and electronic instruments.
                                           → answered · relations.kinds · class=musical instrument

[en] U: which string instruments do you know?
     T: The guitar, the violin, the dulcimer and the «bandurria» (a Spanish
        word; I don't have it in English). → answered · relations.kinds · class=string instrument

[en] U: what types of guitar do you know?
     T: The electric guitar and the classical guitar.
                                           → answered · relations.kinds · class=guitar

[en] U: name some planets
     T: Earth and Mars.                    → answered · relations.kinds · class=planet

[en] U: what kinds of animals do you know?
     T: Mammals and fish.                  → answered · relations.kinds · class=animal

[en] U: list all the instruments you know
     T: String: guitar (electric and classical), violin, dulcimer and
        «bandurria». Wind: flute. Percussion: drum. Electronic: synthesizer.
                                           → answered · relations.kinds · class=musical instrument · depth=all

Given: U1.
[en] U: which string instruments do you know?
     T: The guitar, the violin, the dulcimer, the «bandurria» (a Spanish word)
        and the «zanfona» (a Spanish word you taught me).
                                           → answered · relations.kinds · class=string instrument

[en] U: what dwarf planets do you know?
     T: Pluto, since August 2006.          → answered · relations.kinds · class=dwarf planet
```

#### Edge cases

```text
[es] U: ¿qué tipos de flauta hay?
     T: No conozco tipos de flauta.        → unhandled · relations.kinds · class=flauta
     (known concept, nothing below it: nothing known)

[en] U: what kinds of drums are there?
     T: I don't know any kinds of drum.    → unhandled · relations.kinds · class=drum

[es] U: ¿qué tipos de banco hay?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense   (relations.kinds)
     (no sense has kinds, so structural fit does not decide; rank ties)
[es] U: el de dinero
     T: No conozco tipos de banco.         → unhandled · relations.kinds · class=banco · sense=core:bank

[es] U: ¿qué tipos de cosas hay?
     T: No conozco la palabra «cosas».     → unhandled · relations.kinds · class=cosas
     (not in the fixture; Talos does not list the whole store)

[en] U: what kinds of things are made of wood?
     T: I can't answer that: I don't know what things are made of.
                                           → unhandled · no intent
     (a material question, not a kinds question)
```

### relations.parts

**Recognises** — the parts of something, or whether it has a given part:
«¿qué partes tiene X?», «¿de qué partes consta X?», «¿de qué se compone X?»,
«partes de X», «¿X tiene Y?», «¿lleva X Y?»; "what are the parts of X?",
"what parts does X have?", "what is X made up of?", "does X have a Y?",
"has X got Y?".

**Slots**
- `whole` — `term`, required.
- `part` — `term`, optional; present for the yes/no form.
- `depth` — `choice` (`direct` | `all`), optional; default `direct`.

**Answers** — the list form gives the direct parts (`has_part`, the inverse
of `part_of`), plus parts inherited from the whole's classes, minus those a
more specific negated claim excludes; the inherited and excluded ones are
named as such. With `all`, sub-parts too, nested under their part. The yes/no
form answers «Sí»/"Yes" with path (sub-part or inherited), «No»/"No" with the
contradicting claim, or a 2.4 unhandled negative.

**Does not**
- «¿de qué forma parte X?», «¿X es parte de Y?» → `relations.part_of`.
- «¿de qué está hecha la guitarra?» (material): `unhandled`.
- «¿tienes una guitarra?» (about Talos): `identity.nature` or `unhandled`, never this intent.

#### Examples

```text
[es] U: ¿qué partes tiene una guitarra?
     T: Mástil, caja de resonancia y cuerdas.
                                           → answered · relations.parts · whole=guitarra
     (C19, C20, C21; frets are a sub-part and only appear with "todas")

[es] U: ¿de qué partes consta un violín?
     T: De cuerdas y caja de resonancia.   → answered · relations.parts · whole=violín

[es] U: ¿qué partes tiene la guitarra eléctrica?
     T: Pastillas y, como toda guitarra, mástil y cuerdas. Caja de resonancia,
        no.                                → answered · relations.parts · whole=guitarra eléctrica
     (own C23; inherited C19, C21; C20 excluded by the more specific C24)

[es] U: ¿la guitarra tiene trastes?
     T: Sí: los trastes son parte del mástil, y el mástil es parte de la
        guitarra.                          → answered · relations.parts · whole=guitarra · part=traste

[es] U: ¿la guitarra eléctrica tiene caja de resonancia?
     T: No: aunque las guitarras en general la tienen, la guitarra eléctrica
        no.                                → answered · relations.parts · whole=guitarra eléctrica · part=caja de resonancia
     (inheritance with a more specific exception)

[es] U: ¿la guitarra eléctrica tiene mástil?
     T: Sí: como toda guitarra, tiene mástil.
                                           → answered · relations.parts · whole=guitarra eléctrica · part=mástil

[es] U: ¿de qué se compone el sistema solar?
     T: Conozco tres partes: la Tierra, Marte y Plutón.
                                           → answered · relations.parts · whole=sistema solar

[es] U: dime todas las partes de la guitarra
     T: Mástil (con sus trastes), caja de resonancia y cuerdas.
                                           → answered · relations.parts · whole=guitarra · depth=all
```

```text
[en] U: what are the parts of a guitar?
     T: The neck, the sound box and the strings.
                                           → answered · relations.parts · whole=guitar

[en] U: what parts does a violin have?
     T: Strings and a sound box.           → answered · relations.parts · whole=violin

[en] U: what's an electric guitar made up of?
     T: Pickups and, like any guitar, a neck and strings. No sound box.
                                           → answered · relations.parts · whole=electric guitar

[en] U: does a guitar have frets?
     T: Yes: frets are part of the neck, and the neck is part of the guitar.
                                           → answered · relations.parts · whole=guitar · part=fret

[en] U: does an electric guitar have a sound box?
     T: No: guitars in general do, but an electric guitar doesn't.
                                           → answered · relations.parts · whole=electric guitar · part=sound box

[en] U: has a classical guitar got a neck?
     T: Yes: like any guitar, it has a neck.
                                           → answered · relations.parts · whole=classical guitar · part=neck

[en] U: what's in the solar system?
     T: I know of three parts: Earth, Mars and Pluto.
                                           → answered · relations.parts · whole=solar system
     (a list never claims to be complete; hedge words are kept for confidence, 2.5)
```

#### Edge cases

```text
[es] U: ¿qué partes tiene un tambor?
     T: No conozco las partes del tambor.  → unhandled · relations.parts · whole=tambor

[es] U: ¿el violín tiene trastes?
     T: Nada de lo que sé dice que el violín tenga trastes.
                                           → unhandled · relations.parts · whole=violín · part=traste
     (nothing known, not "no": the fixture has no neck for the violin)

[en] U: does a violin have a neck?
     T: Nothing I know says a violin has a neck.
                                           → unhandled · relations.parts · whole=violin · part=neck

[en] U: what parts does a bank have?
     T: I don't know the parts of a bank (the financial institution). "Bank"
        can also mean the side of a river.
                                           → unhandled · relations.parts · whole=bank · sense=core:bank
     (English "bank" has a strict rank winner, so Talos does not ask; nothing known; the other sense is mentioned)

[es] U: ¿qué partes tiene un banco?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense   (relations.parts)
     (Spanish ties at rank 0 and no sense has parts)
```

### relations.part_of

**Recognises** — what something is part of, or whether it is part of a given
whole: «¿de qué forma parte X?», «¿de qué es parte X?», «¿X forma parte de
Y?», «¿X es parte de Y?», «¿dónde va X?» (only with a part known to the
store); "what is X part of?", "what's X a part of?", "is X part of Y?",
"does X belong to Y?".

**Slots**
- `part` — `term`, required.
- `whole` — `term`, optional; present for the yes/no form.

**Answers** — the list form gives the direct wholes and, after «y por
tanto» / "and so", the derived ones up to the depth limit, with the link that
leads there. Wholes reached only by inheritance are not listed (a fret is
part of *a guitar's* neck, not of every instrument). The yes/no form answers
with path, contradicting claim, or a 2.4 unhandled negative.

**Does not**
- «¿qué partes tiene X?» → `relations.parts`.
- «¿X es un Y?» → `relations.is_a`.
- «¿de qué grupo forma parte Rosalía?» (people, bands): encyclopedic;
  unknown word in this fixture, `unhandled`.

#### Examples

```text
[es] U: ¿de qué forma parte el mástil?
     T: De la guitarra.                    → answered · relations.part_of · part=mástil

[es] U: ¿de qué es parte un traste?
     T: Del mástil, y por tanto de la guitarra.
                                           → answered · relations.part_of · part=traste
     (C22 direct; C22 + C19 derived)

[es] U: ¿la Tierra forma parte del sistema solar?
     T: Sí.                                → answered · relations.part_of · part=Tierra · whole=sistema solar

[es] U: ¿las cuerdas son parte del violín?
     T: Sí.                                → answered · relations.part_of · part=cuerda · whole=violín
     (structural fit: only core:string is part of anything; rank would have chosen "rope")

[es] U: ¿de qué forma parte la pastilla?
     T: La pastilla (de guitarra) forma parte de la guitarra eléctrica.
                                           → answered · relations.part_of · part=pastilla · sense=core:pickup
     (structural fit, sense named)

[es] U: ¿la caja de resonancia es parte de la guitarra eléctrica?
     T: No: las guitarras en general tienen caja de resonancia, pero la
        eléctrica no.                      → answered · relations.part_of · part=caja de resonancia · whole=guitarra eléctrica

[es] U: ¿de qué forma parte una orilla?
     T: Del río.                           → answered · relations.part_of · part=orilla

[es] U: ¿Plutón forma parte del sistema solar?
     T: Sí.                                → answered · relations.part_of · part=Plutón · whole=sistema solar
     (C54 has no interval: its reclassification does not affect it)
```

```text
[en] U: what's a fret part of?
     T: The neck, and so the guitar.       → answered · relations.part_of · part=fret

[en] U: is the neck part of a guitar?
     T: Yes.                               → answered · relations.part_of · part=neck · whole=guitar
     (core:neck_body is rank 0, but only core:neck is part of a guitar: structural fit)

[en] U: is Mars part of the solar system?
     T: Yes.                               → answered · relations.part_of · part=Mars · whole=solar system

[en] U: what is a pickup part of?
     T: An electric guitar.                → answered · relations.part_of · part=pickup

[en] U: are strings part of a violin?
     T: Yes.                               → answered · relations.part_of · part=string · whole=violin

[en] U: is a sound box part of an electric guitar?
     T: No: guitars in general have one, but an electric guitar doesn't.
                                           → answered · relations.part_of · part=sound box · whole=electric guitar

[en] U: what is a bank part of?
     T: A bank (of a river) is part of a river.
                                           → answered · relations.part_of · part=bank · sense=core:riverbank
     (rank would pick core:bank, but structural fit comes first and only the riverbank is part of anything)
```

#### Edge cases

```text
[es] U: ¿el mástil es parte del violín?
     T: Nada de lo que sé dice que el violín tenga mástil.
                                           → unhandled · relations.part_of · part=mástil · whole=violín

[en] U: what is a flute part of?
     T: I don't know of anything a flute is part of.
                                           → unhandled · relations.part_of · part=flute

[es] U: ¿la guitarra forma parte de la orquesta?
     T: No conozco la palabra «orquesta».  → unhandled · relations.part_of · whole=orquesta

[en] U: is a fret part of an electric guitar?
     T: Yes: frets are part of the neck, and an electric guitar, like any
        guitar, has a neck.                → answered · relations.part_of · part=fret · whole=electric guitar
     (yes/no form may combine part_of with inheritance on the whole; path stated)
```

### relations.causes

**Recognises** — what something causes, what causes it, or whether one thing
causes another: «¿qué causa X?», «¿qué provoca X?», «¿por qué hay X?», «¿por
qué llueve?», «¿a qué se debe X?», «¿de dónde viene X?», «¿qué consecuencias
tiene X?», «¿qué produce X?», «¿X provoca Y?», «¿X causa Y?»; "what causes
X?", "why does it X?", "why are there Xs?", "what does X cause?", "what are
the effects of X?", "what does X lead to?", "does X cause Y?".

Spanish «¿qué provoca / qué causa / qué produce X?» is genuinely ambiguous
(what X causes, or what causes X). Talos does not pick one reading: it answers
**both directions** that have claims, causes first.

**Slots**
- `topic` — `term`, required.
- `direction` — `choice` (`causes_of` | `effects_of` | `both`), required, set
  by the phrasing; `both` for the ambiguous Spanish forms.
- `other` — `term`, optional; present for the yes/no form.

**Answers** — direct causes or effects, each with its confidence wording;
derived ones are only given in the yes/no form, or as the next link when the
user asks «¿y eso qué provoca?». The yes/no form gives the path and the band
of its product (2.5). A negated causal claim gives a contradicted "no", with
the positive cause if one is known.

**Does not**
- «¿por qué la guitarra tiene cuerdas?», "why is the sky blue?" (why-questions
  about purpose or properties): `unhandled`, no intent.
- «¿por qué no me contestas?» (about Talos): reflector.
- «¿qué pasa si…?» (hypotheticals): `unhandled`.

#### Examples

```text
[es] U: ¿por qué llueve?
     T: La provocan las tormentas y, por lo que sé, también las nubes.
                                           → answered · relations.causes · topic=lluvia · direction=causes_of
     (C35 hedged 0.8, C37 plain 0.95; "llueve" → llover → core:rain)

[es] U: ¿qué provoca la lluvia?
     T: A la lluvia la provocan las tormentas y, por lo que sé, las nubes. Y
        la lluvia puede que provoque inundaciones, aunque no lo sé con certeza.
                                           → answered · relations.causes · topic=lluvia · direction=both
     (ambiguous Spanish: both directions; C36 is weak)

[es] U: ¿qué consecuencias tiene el calor?
     T: La evaporación.                    → answered · relations.causes · topic=calor · direction=effects_of

[es] U: ¿a qué se deben las inundaciones?
     T: Puede que a la lluvia, pero no lo sé con certeza.
                                           → answered · relations.causes · topic=inundación · direction=causes_of
     (C36: 0.5, weak band)

[es] U: ¿el calor provoca lluvia?
     T: Diría que sí, de forma indirecta: el calor provoca evaporación, la
        evaporación forma nubes y las nubes traen lluvia.
                                           → answered · relations.causes · topic=calor · other=lluvia
     (three links, ≈0.71: hedged)

[es] U: ¿el frío provoca resfriados?
     T: No: el resfriado no lo causa el frío, sino un virus.
                                           → answered · relations.causes · topic=frío · other=resfriado
     (contradicted by C39; C38 named)

[es] U: ¿de dónde viene el catarro?
     T: De un virus.                       → answered · relations.causes · topic=catarro · direction=causes_of

Given: U1.
[es] U: ¿qué produce el calor?
     T: Evaporación y, según me enseñaste, sequía.
                                           → answered · relations.causes · topic=calor · direction=both
     (nothing causes heat in the store, so only effects are given; taught claim attributed)
```

```text
[en] U: why does it rain?
     T: Storms cause rain, and as far as I know clouds bring it too.
                                           → answered · relations.causes · topic=rain · direction=causes_of

[en] U: what causes floods?
     T: Possibly rain, but I'm not sure.   → answered · relations.causes · topic=flood · direction=causes_of

[en] U: what does heat cause?
     T: Evaporation.                       → answered · relations.causes · topic=heat · direction=effects_of

[en] U: what are the effects of a storm?
     T: Rain.                              → answered · relations.causes · topic=storm · direction=effects_of

[en] U: does heat cause rain?
     T: I'd say so, indirectly: heat causes evaporation, evaporation forms
        clouds, and clouds bring rain.     → answered · relations.causes · topic=heat · other=rain

[en] U: does cold weather give you a cold?
     T: No: a cold isn't caused by cold weather but by a virus.
                                           → answered · relations.causes · topic=cold weather · other=common cold
     ("cold weather" → core:cold; "a cold" after "give you" → structural fit: only core:common_cold is caused by anything)

[en] U: what's the cause of the common cold?
     T: A virus.                           → answered · relations.causes · topic=common cold · direction=causes_of

Given: U1.
[en] U: what does heat lead to?
     T: Evaporation and, as you taught me, drought.
                                           → answered · relations.causes · topic=heat · direction=effects_of
     (taught in Spanish, asked in English: the claim is between concepts)
```

#### Edge cases

```text
[es] U: ¿la lluvia provoca terremotos?
     T: Nada de lo que sé dice que la lluvia provoque terremotos.
                                           → unhandled · relations.causes · topic=lluvia · other=terremoto
     (both words known; nothing known, not "no")

[en] U: does heat cause floods?
     T: Possibly, but I'm not sure: heat causes evaporation, evaporation forms
        clouds, clouds bring rain, and rain may cause floods.
                                           → answered · relations.causes · topic=heat · other=flood
     (four links, ≈0.35: weak; the full path is given because the answer is weak)

Given: U2.
[es] U: ¿el frío provoca resfriados?
     T: Sí, según me enseñaste.            → answered · relations.causes · topic=frío · other=resfriado
     (the user claim overrides C39 at query time; the pack is unchanged)

[es] U: ¿qué provoca la lluvia?
     T: A la lluvia la provocan las tormentas y, por lo que sé, las nubes. Y
        la lluvia puede que provoque inundaciones, aunque no lo sé con certeza.
                                           → answered · relations.causes · direction=both
[es] U: ¿y las inundaciones qué provocan?
     T: No sé qué provocan las inundaciones.
                                           → unhandled · relations.causes · topic=inundación · direction=effects_of
     (the follow-up fixes the direction; nothing known)

[es] U: ¿por qué la guitarra tiene seis cuerdas?
     T: Eso no lo sé.                      → unhandled · no intent
     (a why-question about a property, not a cause in the store; never reflected)

[en] U: why is the sky blue?
     T: I don't know that.                 → unhandled · no intent
```

### relations.about

**Recognises** — an open request for what Talos knows about something:
«¿qué sabes de X?», «¿qué sabes sobre X?», «háblame de X», «cuéntame algo de
X», «¿qué me puedes decir de X?»; "what do you know about X?", "tell me
about X", "tell me something about X", "what can you tell me about X?".

**Slots**
- `topic` — `term`, required.

**Answers** — up to five items of direct knowledge about the concept, in this
order: its gloss (or, without one, its class), its classes, its parts, its
kinds, what it is part of, what it causes and what causes it, and past claims
marked as past. A list of parts or of kinds counts as one item. A class or a
part that the gloss already states is not repeated. Taught claims are attributed.
Derived claims are not listed. At most two sentences; if there are more than
five items, the reply ends by saying how many more it knows (a number, never
«y más cosas»).

**Does not**
- «¿qué sabes de mí?» → `preferences.get`.
- «¿qué sabes hacer?» → `help.capabilities`.
- «háblame de ti» → `identity.who`.
- «cuéntame un chiste sobre guitarras», «escríbeme algo sobre el violín»:
  generative, `unhandled`.
- «¿qué te he enseñado sobre X?» → `teach.list`.

#### Examples

```text
[es] U: ¿qué sabes de la guitarra?
     T: Es un instrumento de cuerda con mástil y caja de resonancia, que se
        toca punteando o rasgueando las cuerdas. Conozco dos tipos: la
        eléctrica y la clásica.
                                           → answered · relations.about · topic=guitarra
     (gloss, then kinds: the class and the parts are already in the gloss, so not repeated)

[es] U: háblame del violín
     T: Es un instrumento de cuerda que se toca con arco, apoyado entre el
        hombro y la barbilla; tiene cuerdas y caja de resonancia.
                                           → answered · relations.about · topic=violín

[es] U: cuéntame algo de Plutón
     T: Es un planeta enano y forma parte del sistema solar. Hasta agosto de
        2006 se consideraba planeta.       → answered · relations.about · topic=Plutón
     (the expired C48 is listed as past)

[es] U: ¿qué sabes sobre la lluvia?
     T: La provocan las tormentas y, por lo que sé, las nubes; y puede que
        provoque inundaciones, aunque no lo sé con certeza.
                                           → answered · relations.about · topic=lluvia

[es] U: ¿qué me puedes decir de la ballena?
     T: Es un mamífero, no un pez.         → answered · relations.about · topic=ballena
     (negated claims are knowledge too)

Given: U1.
[es] U: ¿qué sabes de la zanfona?
     T: Solo lo que me enseñaste: que es un instrumento de cuerda.
                                           → answered · relations.about · topic=zanfona
```

```text
[en] U: what do you know about guitars?
     T: A guitar is a string instrument with a fretted neck and a hollow body,
        played by plucking or strumming. I know two kinds: electric and
        classical.
                                           → answered · relations.about · topic=guitar

[en] U: tell me about the violin
     T: It's a string instrument played with a bow and held under the chin;
        it has strings and a sound box.    → answered · relations.about · topic=violin

[en] U: what can you tell me about Pluto?
     T: It's a dwarf planet and part of the solar system. Until August 2006 it
        was counted as a planet.           → answered · relations.about · topic=Pluto

[en] U: tell me something about rain
     T: Storms cause it and, as far as I know, clouds bring it; it may cause
        floods, but I'm not sure.          → answered · relations.about · topic=rain

[en] U: what do you know about banks?
     T: A bank is a business that keeps money, lends it and offers other
        financial services; it's a financial institution. "Bank" can also
        mean the side of a river.          → answered · relations.about · topic=bank · sense=core:bank

Given: U1.
[en] U: what do you know about the theremin?
     T: Only what you taught me: it's an electronic instrument.
                                           → answered · relations.about · topic=theremin
```

#### Edge cases

```text
[es] U: ¿qué sabes de física cuántica?
     T: No conozco «física cuántica».      → unhandled · relations.about · topic=física cuántica

[es] U: ¿qué sabes del banco?
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense   (relations.about)

[en] U: tell me about yourself
     T: (identity.who answer)              → answered · identity.who
     (not this intent)

[es] U: ¿qué sabes de los trastes?
     T: Que son parte del mástil. No tengo una definición en español; en
        inglés se dice «fret».             → answered · relations.about · topic=traste (from "trastes")
     (no Spanish gloss, so the part claim leads)

[en] U: what do you know about the bandurria?
     T: «Bandurria» is a Spanish word: it's a string instrument. I have no
        English word or English definition for it.
                                           → answered · relations.about · topic=bandurria
     (Spanish-only lexeme; the Spanish gloss is not shown unasked)
```

### relations.provenance

**Recognises** — how sure Talos is, or where it learnt something: about the
last answer («¿cómo lo sabes?», «¿seguro?», «¿estás seguro?», «¿quién te lo
ha dicho?», «¿de dónde lo has sacado?», «¿en qué te basas?», «¿cómo de seguro
estás?», «¿y en porcentaje?»; "how do you know?", "are you sure?", "who told
you?", "where did you get that?", "how sure are you?", "as a number?") or
about a named claim («¿cómo sabes que la zanfona es un instrumento de
cuerda?», "where did you learn that Pluto is a dwarf planet?").

**Slots**
- `claim` — `term`, optional; default the claim or path of the last answer
  (2.8). Asked if there is none.
- `numeric` — `choice` (yes/no), optional; default no; yes when the user asks
  for a number or a percentage.

**Answers** — for each claim used: its source (the pack by name, or the user
with the date it was taught), and for a derived answer the full path. How sure
Talos is, in the words of 2.5; with `numeric`, the confidence as a rounded
percentage and what it is made of. If a user claim overrides a pack claim,
both are mentioned. A gloss's source is given for definitions; a lexeme's for
translations.

**Does not**
- «¿qué te he enseñado?» → `teach.list`.
- «¿cómo funcionas?» → `identity.nature`.
- «¿estás seguro de que son las 18:42?» (about another skill's answer): the
  clock is not knowledge; the reply says the time comes from the clock Talos
  is given, never a confidence. That is a complete answer, so `answered`
  (README, "Which status an outcome gets").

#### Examples

```text
[es] U: ¿el violín es un instrumento musical?
     T: Sí: el violín es un instrumento de cuerda, y los instrumentos de
        cuerda son instrumentos musicales. → answered · relations.is_a
[es] U: ¿cómo lo sabes?
     T: Las dos cosas vienen del paquete core, lo que traigo de serie, y las
        tengo por seguras.                 → answered · relations.provenance · claim=last

[es] U: ¿el calor provoca lluvia?
     T: Diría que sí, de forma indirecta: el calor provoca evaporación, la
        evaporación forma nubes y las nubes traen lluvia.
                                           → answered · relations.causes
[es] U: ¿seguro?
     T: No del todo. Todo viene del paquete core; lo más flojo es que las
        nubes traigan lluvia.              → answered · relations.provenance · claim=last
[es] U: ¿y en porcentaje?
     T: Un 71 % aproximadamente: es el producto de las tres relaciones del
        camino (100 %, 90 % y 80 %), con un pequeño descuento por cada paso.
                                           → answered · relations.provenance · claim=last · numeric=yes

Given: U1.
[es] U: ¿cómo sabes que la zanfona es un instrumento de cuerda?
     T: Me lo enseñaste tú el 10 de marzo. Lo tengo por bastante seguro, pero
        no lo he comprobado con nada más.  → answered · relations.provenance · claim=zanfona is_a instrumento de cuerda

Given: U2.
[es] U: ¿quién te ha dicho que el frío provoca resfriados?
     T: Tú, el 13 de marzo. Lo que traigo de serie dice lo contrario; uso lo
        tuyo porque me pediste que lo cambiara.
                                           → answered · relations.provenance · claim=frío causes resfriado

[es] U: ¿qué es una guitarra?
     T: Una guitarra es un instrumento de cuerda con mástil y caja de
        resonancia, que se toca punteando o rasgueando las cuerdas.
                                           → answered · define.word
[es] U: ¿de dónde has sacado esa definición?
     T: Del paquete core, el vocabulario básico que traigo de serie.
                                           → answered · relations.provenance · claim=last (gloss)

[es] U: ¿cuánto de seguro estás de que Plutón es un planeta enano?
     T: Totalmente: lo dice el paquete core, desde agosto de 2006.
                                           → answered · relations.provenance · claim=Plutón is_a planeta enano
```

```text
[en] U: are violins musical instruments?
     T: Yes: a violin is a string instrument, and string instruments are
        musical instruments.               → answered · relations.is_a
[en] U: how do you know?
     T: Both come from the core pack, what I came with, and I'm sure of them.
                                           → answered · relations.provenance · claim=last

[en] U: what causes floods?
     T: Possibly rain, but I'm not sure.   → answered · relations.causes
[en] U: how sure are you?
     T: Not very: the core pack gives that link only low confidence.
                                           → answered · relations.provenance · claim=last
[en] U: as a number?
     T: 50%.                               → answered · relations.provenance · claim=last · numeric=yes

Given: U1.
[en] U: who told you a theremin is an electronic instrument?
     T: You did, on 11 March.              → answered · relations.provenance · claim=theremin is_a electronic instrument

[en] U: is an electric guitar a device?
     T: Yes, in four steps: an electric guitar is a guitar, …, and a musical
        instrument is a device.            → answered · relations.is_a
[en] U: where did you get that?
     T: From the core pack: an electric guitar is a guitar, a guitar is a
        string instrument, a string instrument is a musical instrument, and a
        musical instrument is a device.    → answered · relations.provenance · claim=last
     (the full path, which the answer summarised)

[en] U: how sure are you that a whale isn't a fish?
     T: Completely: the core pack says so.
                                           → answered · relations.provenance · claim=whale not is_a fish

Given: U2.
[en] U: where did you learn that cold weather causes colds?
     T: You taught me on 13 March. What I came with says the opposite; I use
        yours because you asked me to.     → answered · relations.provenance
```

#### Edge cases

```text
[es] U: ¿cómo lo sabes?
     T: ¿El qué?                           → asks · claim   (relations.provenance)
     (new conversation: no last answer)

[en] U: is a drum a string instrument?
     T: Nothing I know says a drum is a string instrument. I know it's a
        percussion instrument.             → unhandled · relations.is_a
[en] U: are you sure?
     T: I'm sure I have nothing saying so. That a drum is a percussion
        instrument comes from the core pack.
                                           → answered · relations.provenance · claim=last
     (provenance of an absence: what was searched and what was found)

[es] U: ¿la guitarra es una entidad?
     T: No lo encuentro a menos de seis pasos: la relación, si existe, está
        más lejos.                         → unhandled · relations.is_a
[es] U: ¿seguro?
     T: Solo busco hasta seis pasos para no perderme. Llego hasta «entidad
        física» y ahí paro.                → answered · relations.provenance · claim=last

[en] U: what's the time?
     T: It's 18:42.                        → answered · clock.now
[en] U: are you sure?
     T: The time comes from the clock I'm given; it isn't something I know.
                                           → answered · relations.provenance
     (another skill's answer has no knowledge provenance)
```

---

## teach

How the user adds to, corrects and inspects what Talos knows, and teaches it
their own phrasings. Teaching is the **only** way Talos learns: it learns what
it is told in a teaching form and nothing it overhears (KNOWLEDGE.md §6 and
§8, ADR 0005). An ordinary statement, however fact-like, is never stored,
never used as context for later answers, and never acknowledged as learnt.
Taught facts go to `user.db` with the user as source and confidence 0.95, are
attributed whenever they are used (2.5), and are listed, exportable and
reversible. Packs are never modified: a correction of pack knowledge is a user
claim that overrides it at query time, and forgetting pack knowledge stores a
tombstone. Talos asks before overriding anything it already knows. Writes
happen only on these intents, and only once any question has been answered.
A write is never made on classifier confidence alone (DESIGN.md §11): when a
`teach.*` intent was recognised by the classifier and not by a rule, Talos
first asks for a yes/no confirmation that restates what it would store or
forget.

### Teaching forms

A teaching request is a **cue** followed by a **statement of a supported
shape**. Both are needed.

| Cue | es | en |
|---|---|---|
| explicit | «aprende: …», «aprende que …», «apréndete que …», «apunta que …», «apúntate que …», «toma nota: …», «quiero enseñarte que …», «te enseño una cosa: …» | "teach: …", "learn: …", "learn that …", "note that …", "take note: …", "I want to teach you that …", "here's something for you to learn: …" |
| remember | «recuerda que …», only when the statement is a supported shape about something other than the user | "remember that …", same condition |

| Shape | es | en |
|---|---|---|
| is a | «X es un/una Y», «los X son Y», «X es un tipo de Y» | "X is a Y", "Xs are Ys", "X is a kind of Y" |
| is not a | «X no es un/una Y» | "X is not a Y", "X isn't a Y" |
| part | «X es parte de Y», «X forma parte de Y», «X tiene Y», «X lleva Y» | "X is part of Y", "X has Y", "X has got Y" |
| not part | «X no tiene Y», «X no lleva Y» | "X has no Y", "X doesn't have Y" |
| causes | «X provoca / causa / produce Y» | "X causes Y", "X leads to Y" |
| does not cause | «X no provoca / no causa Y» | "X doesn't cause Y" |
| meaning | ««X» significa …», ««X» quiere decir …» | "'X' means …" |
| synonym | ««X» es otra forma de decir «Y»», ««X» es lo mismo que «Y»» | "'X' is another word for 'Y'", "'X' is the same as 'Y'" |
| translation | «X en inglés se dice Y», «X en inglés es Y» | "the Spanish for X is Y", "X in Spanish is Y" |

Everything else after a cue — adjectives and opinions («molan», "are cool"),
quantities, dates and time limits («hasta 2002…»), comparisons, statements
about people — is an unsupported shape: Talos says what it can learn and
stores nothing.

### teach.assert

**Recognises** — a teaching cue followed by a statement, in either language;
the statement may be in quotes or bare.

**Slots**
- `statement` — `term`, required. Asked if missing («aprende» alone).
- From the statement: `subject`, `predicate`, `object` (or `gloss`,
  `synonym`, `word` + `language`), `negated`.

**Answers** — in this order of checks:

1. **Unsupported shape** → says what kinds of things it can learn; nothing
   stored (`unhandled`).
2. **Words.** An unknown *subject* is a new word: a lexeme in the language
   it was taught in, a sense and a new `user:` concept are created, and the
   reply says the word is new. An unknown *object* is asked about (near miss
   per 2.1, otherwise «¿lo aprendo como algo nuevo?»), because a new class is
   more often a typo than a teaching. A polysemous word goes through 2.2 and
   is asked on a tie. A new word is stored as typed, without guessing its
   lemma.
3. **Already known.** If the claim already holds — directly or derived, from
   any source — Talos says it already knew it (and from whom, if taught) and
   stores nothing.
4. **Contradiction** (KNOWLEDGE.md §4): the statement is the negation of a
   stored claim (pack or user), or of a derived one, or would close a cycle in
   `is_a`. Talos states what it knows that conflicts, and:
   - against a **pack claim**: asks whether to hold the user's version from
     now on. Yes → the user claim is stored and overrides the pack claim at
     query time; the pack is untouched. No → nothing changes.
   - against a **user claim**: asks whether to replace it. Yes → the old one
     is retracted, the new one stored. No → nothing changes.
   - against a **derived claim**: names the path and asks which link is wrong
     (a `choice`, with «ninguna»/"none" to cancel). The chosen link is
     overridden as above and the statement stored.
   - a **cycle**: refuses and says why; nothing stored (`unhandled`). The user
     can forget one of the claims first.
5. **An exception** — a negated claim about a subclass where the class has the
   positive one — is not a contradiction: it is stored without asking, and the
   reply says it is an exception.
6. Otherwise the claim is stored and the reply restates it in the
   conversation language.

The audit records every write. The reply never says how the fact will be
used, and never thanks the user.

**Does not**
- **Ordinary statements are not teaching.** «las guitarras molan», "guitars
  are cool", «la zanfona es un instrumento de cuerda», "whales are mammals"
  without a cue are statements: they go to the reflector (typically
  `deflected`, see [reflector.md](reflector.md)), nothing is stored, nothing
  is remembered as context, and Talos does not hint that they could be taught.
- «recuerda que me llamo Ana», "remember I live in Leeds" → `preferences.set`.
- «cuando diga X quiero decir Y» → `teach.phrase`.
- «aprende a tocar la guitarra», "learn to play chess" (learning a skill):
  `unhandled`, no intent.
- «enséñame algo», "teach me something" (the user wants to be taught):
  `fortune.tell` if phrased as a saying, otherwise `unhandled`.
- Pasted text to extract facts from: `unhandled` (no free-text extraction).

#### Examples

```text
[es] U: aprende: la zanfona es un instrumento de cuerda
     T: Aprendido: la zanfona es un instrumento de cuerda. «Zanfona» es una
        palabra nueva para mí.             → answered · teach.assert · subject=zanfona (new) · predicate=is_a · object=instrumento de cuerda

[es] U: apunta que el clavijero es parte del mástil
     T: Apuntado: el clavijero es parte del mástil. No conocía «clavijero».
                                           → answered · teach.assert · subject=clavijero (new) · predicate=part_of · object=mástil

[es] U: aprende que el calor provoca sequía
     T: Aprendido: el calor provoca sequía.
                                           → answered · teach.assert · subject=calor · predicate=causes · object=sequía

[es] U: toma nota: las castañuelas son instrumentos de percusión
     T: Apuntado: las castañuelas son instrumentos de percusión. No conocía
        «castañuelas».                     → answered · teach.assert · subject=castañuelas (new) · predicate=is_a · object=instrumento de percusión
     (stored as typed: Talos does not guess a singular for a new word)

[es] U: aprende: «sinte» es otra forma de decir sintetizador
     T: Aprendido: «sinte» quiere decir sintetizador.
                                           → answered · teach.assert · synonym=sinte · of=sintetizador
[es] U: ¿qué es un sinte?
     T: Un sinte es un instrumento electrónico que genera y modifica sonidos.
                                           → answered · define.word · word=sinte · sense=core:synthesizer

Given: U1.
[es] U: aprende: «zanfona» significa instrumento de cuerda en el que una
        rueda, movida con una manivela, frota las cuerdas
     T: Aprendido: ya tengo una definición de «zanfona».
                                           → answered · teach.assert · word=zanfona · gloss(es)

Given: U1.
[es] U: aprende: zanfona en inglés se dice hurdy-gurdy
     T: Aprendido: «zanfona» en inglés es «hurdy-gurdy».
                                           → answered · teach.assert · word=zanfona · translation(en)=hurdy-gurdy

[es] U: aprende: un banco es una empresa
     T: ¿Qué banco: la entidad financiera, el asiento o el grupo de peces?
                                           → asks · sense   (teach.assert)
[es] U: el de dinero
     T: Aprendido: un banco (entidad financiera) es una empresa.
                                           → answered · teach.assert · subject=banco · sense=core:bank · predicate=is_a · object=empresa

Given: U1.
[es] U: ¿el theremin es un instrumento musical?
     T: Sí: según me enseñaste, el theremin es un instrumento electrónico, y
        los instrumentos electrónicos son instrumentos musicales.
                                           → answered · relations.is_a · subject=theremin · class=instrumento musical
     (taught in English on 11 March, asked in Spanish)
```

```text
[en] U: teach: a theremin is an electronic instrument
     T: Got it: a theremin is an electronic instrument. "Theremin" is a new
        word for me.                       → answered · teach.assert · subject=theremin (new) · predicate=is_a · object=electronic instrument

[en] U: learn that a tuning peg is part of a guitar
     T: Got it: a tuning peg is part of a guitar. I didn't know "tuning peg".
                                           → answered · teach.assert · subject=tuning peg (new) · predicate=part_of · object=guitar

[en] U: note that heat causes drought
     T: Noted: heat causes drought.        → answered · teach.assert · subject=heat · predicate=causes · object=drought

[en] U: teach: "synth" is another word for synthesizer
     T: Got it: "synth" means synthesizer. → answered · teach.assert · synonym=synth · of=synthesizer

Given: U1.
[en] U: teach: "theremin" means an electronic instrument you play by moving
        your hands near it, without touching it
     T: Got it: I now have a definition of "theremin".
                                           → answered · teach.assert · word=theremin · gloss(en)

Given: U1.
[en] U: teach: the Spanish for theremin is theremín
     T: Got it: "theremin" in Spanish is «theremín».
                                           → answered · teach.assert · word=theremin · translation(es)=theremín

[en] U: teach: a bank is a company
     T: Got it: a bank (the financial institution) is a company.
                                           → answered · teach.assert · subject=bank · sense=core:bank · predicate=is_a · object=company
     (English "bank" has a rank winner: no question; the sense is named)

[en] U: teach: a fretless guitar is a guitar
     T: Got it: a fretless guitar is a guitar. "Fretless guitar" is a new word
        for me.                            → answered · teach.assert · subject=fretless guitar (new) · predicate=is_a · object=guitar
[en] U: teach: a fretless guitar has no frets
     T: Got it: a fretless guitar has no frets, unlike guitars in general.
                                           → answered · teach.assert · subject=fretless guitar · predicate=has_part · object=fret · negated
     (an exception to an inherited part: stored without asking)

Given: U1.
[en] U: is a zanfona a string instrument?
     T: Yes, you taught me that. ("Zanfona" is the Spanish word; I don't have
        an English one.)                   → answered · relations.is_a · subject=zanfona · class=string instrument
     (taught in Spanish on 10 March, asked in English)
```

#### Edge cases

```text
[es] U: las guitarras molan
     T: (respuesta del reflector, p. ej. «¿Ah, sí?»)
                                           → deflected · no intent · nothing stored
[es] U: ¿qué te he enseñado?
     T: Todavía no me has enseñado nada.   → answered · teach.list
     (an opinion without a cue is not teaching)

[en] U: guitars are cool
     T: (reflector reply, e.g. "Are they?")
                                           → deflected · no intent · nothing stored

[es] U: la zanfona es un instrumento de cuerda
     T: (respuesta del reflector)          → deflected · no intent · nothing stored
[es] U: ¿la zanfona es un instrumento de cuerda?
     T: No conozco la palabra «zanfona».   → unhandled · relations.is_a · subject=zanfona
     (a fact-shaped statement without a cue is not learnt either, not even for this conversation)

[es] U: aprende que las guitarras molan
     T: Eso no sé aprenderlo. Aprendo qué es algo, de qué forma parte, qué
        partes tiene, qué provoca, qué significa una palabra y cómo se dice en
        el otro idioma.                    → unhandled · teach.assert
     (a cue, but an unsupported shape)

[en] U: teach: guitars are instruments
     T: I already knew that: a guitar is a string instrument, and string
        instruments are musical instruments.
                                           → answered · teach.assert · nothing stored
     (already derivable)

[es] U: aprende: la guitarra eléctrica tiene pastillas
     T: Eso ya lo sabía.                   → answered · teach.assert · nothing stored

Given: U1.
[es] U: aprende: la zanfona es un instrumento de cuerda
     T: Ya me lo enseñaste el 10 de marzo. → answered · teach.assert · nothing stored

[en] U: teach: a whale is a fish
     T: That clashes with what I came with: a whale is not a fish, it's a
        mammal. Do you want me to hold that a whale is a fish from now on?
                                           → asks · confirm   (teach.assert)
[en] U: no
     T: Okay, I'll leave it as it was.     → answered · teach.assert · nothing stored

[es] U: aprende: la guitarra eléctrica tiene caja de resonancia
     T: Eso choca con lo que traigo de serie: la guitarra eléctrica no tiene
        caja de resonancia. ¿Quieres que a partir de ahora considere que sí la
        tiene?                             → asks · confirm   (teach.assert)
[es] U: sí
     T: Hecho: a partir de ahora, la guitarra eléctrica tiene caja de
        resonancia.                        → answered · teach.assert · overrides=C24
[es] U: ¿la guitarra eléctrica tiene caja de resonancia?
     T: Sí, según me enseñaste.            → answered · relations.parts
     (the pack is unchanged; the user claim wins at query time)

Given: U1.
[en] U: teach: a theremin is not an electronic instrument
     T: You taught me the opposite on 11 March: that a theremin is an
        electronic instrument. Shall I replace it?
                                           → asks · confirm   (teach.assert)
[en] U: yes
     T: Done: a theremin is not an electronic instrument.
                                           → answered · teach.assert · retracts=user claim of 11 March

[es] U: aprende: la guitarra no es un instrumento musical
     T: Lo que sé dice lo contrario, en dos pasos: la guitarra es un
        instrumento de cuerda, y los instrumentos de cuerda son instrumentos
        musicales. ¿Cuál de las dos está mal?
                                           → asks · choice   (teach.assert)
[es] U: la primera
     T: Hecho: la guitarra no es un instrumento de cuerda ni un instrumento
        musical.                           → answered · teach.assert · overrides=C01
     (negation of a derived claim: the chosen link is overridden, so the store stays consistent)

[es] U: aprende: un instrumento musical es un tipo de guitarra
     T: No puedo: la guitarra ya es un tipo de instrumento musical, y una cosa
        no puede ser un tipo de sí misma.  → unhandled · teach.assert · cycle
     (cycles are refused, not asked)

[es] U: aprende: la zanfona es un instrumetno de cuerda
     T: No conozco «instrumetno de cuerda». ¿Te refieres a «instrumento de
        cuerda»?                           → asks · object (yes/no)
[es] U: sí, perdona
     T: Aprendido: la zanfona es un instrumento de cuerda. «Zanfona» es una
        palabra nueva para mí.             → answered · teach.assert

[es] U: aprende: la zanfona es un cacharro
     T: No conozco «cacharro». ¿Lo aprendo como algo nuevo?
                                           → asks · object (yes/no)
[es] U: vale
     T: Aprendido: la zanfona es un cacharro. «Zanfona» y «cacharro» son
        palabras nuevas para mí.           → answered · teach.assert

[es] U: aprende: hasta 2002 la moneda de España era la peseta
     T: Eso no sé aprenderlo: no aprendo cosas con fecha.
                                           → unhandled · teach.assert
     (taught claims have no validity interval)

[en] U: teach:
     T: What should I learn?               → asks · statement

[es] U: aprende a tocar la guitarra
     T: Eso no lo puedo hacer.             → unhandled · no intent
     (learning a skill, not a fact)

[en] U: remember that I live in Leeds
     T: (preferences.set answer)           → answered · preferences.set
     (about the user: not teaching)
```

### teach.forget

**Recognises** — retracting something taught or known: «olvida que …»,
«olvídate de que …», «olvida lo de X», «olvida lo que te dije de X», «borra
que …», «olvida lo último que te he enseñado», «olvida todo lo que te he
enseñado», «eso no es verdad, olvídalo» / «eso está mal, bórralo» (about the
last answer); "forget that …", "forget what I told you about X", "unlearn
that …", "delete the last thing I taught you", "forget everything I taught
you", "that's wrong, forget it". A bare «olvídalo» / "forget it" is a
cancellation of an open question, or an acknowledgement when nothing is open
([conversation.md](conversation.md) 3.5), never this intent.

**Slots**
- `target` — `term`, required: a statement, a topic word, a taught phrase
  (quoted), `last` (the last thing taught), `all`, or the claim of the last
  answer.

**Answers**
- A **user claim**: retracted at once (the request is the confirmation); the
  reply restates what was forgotten. A user-created word with nothing left is
  forgotten too, and the reply says so.
- A **pack claim**: asks first, naming the claim; yes → a tombstone in
  `user.db`, and the pack is unchanged. If a user claim overrode it, forgetting
  the user claim brings the pack claim back, and the reply says so.
- A **derived claim**: names the path and asks which link to forget.
- A **topic** («lo de la zanfona»): all the user's claims about it; asks first
  if there is more than one. Pack claims are never forgotten by topic, only
  one at a time.
- **All**: asks first, with the count; it covers user claims and taught
  phrases, not tombstones.
- A **phrase**: the mapping is removed.
- **Nothing matching**: says there was nothing saying that (`answered`:
  there is nothing to undo).

**Does not**
- «olvida mi nombre», «olvida dónde vivo» → `preferences.forget`.
- «olvídalo» with an open question → cancellation.
- «olvida esta conversación»: `unhandled` (conversations are not stored).

#### Examples

```text
Given: U1.
[es] U: olvida que la zanfona es un instrumento de cuerda
     T: Olvidado. Como no sabía nada más de la zanfona, también he olvidado
        la palabra.                        → answered · teach.forget · target=zanfona is_a instrumento de cuerda

Given: U1.
[es] U: olvida lo último que te he enseñado
     T: Olvidado: que el calor provoca sequía.
                                           → answered · teach.forget · target=last

Given: U1.
[es] U: borra lo del theremin
     T: Olvidado: que el theremin es un instrumento electrónico, y la palabra
        «theremin».                        → answered · teach.forget · target=theremin (topic, one claim)

[es] U: olvida que la ballena es un mamífero
     T: Eso lo traigo de serie. ¿Seguro que quieres que lo olvide?
                                           → asks · confirm   (teach.forget)
[es] U: sí
     T: Hecho: ya no considero que la ballena sea un mamífero.
                                           → answered · teach.forget · tombstone=C40

Given: U1.
[es] U: olvida todo lo que te he enseñado
     T: Son 3 cosas. ¿Las olvido todas?    → asks · confirm   (teach.forget)
[es] U: venga
     T: Hecho: he olvidado todo lo que me habías enseñado.
                                           → answered · teach.forget · target=all · count=3

Given: U3.
[es] U: olvida lo de «tira»
     T: Olvidado: «tira» ya no significa tirar un dado.
                                           → answered · teach.forget · target=phrase «tira»

Given: U2.
[es] U: ¿el frío provoca resfriados?
     T: Sí, según me enseñaste.            → answered · relations.causes
[es] U: eso no es verdad, olvídalo
     T: Olvidado. Vuelvo a lo que traía de serie: el frío no causa el
        resfriado.                         → answered · teach.forget · target=last answer (user claim)
```

```text
Given: U1.
[en] U: forget that a theremin is an electronic instrument
     T: Forgotten. I knew nothing else about the theremin, so I've forgotten
        the word as well.                  → answered · teach.forget · target=theremin is_a electronic instrument

Given: U1.
[en] U: delete the last thing I taught you
     T: Forgotten: that heat causes drought.
                                           → answered · teach.forget · target=last

Given: U1.
[en] U: forget what I told you about the zanfona
     T: Forgotten: that a zanfona is a string instrument, and the word
        "zanfona".                         → answered · teach.forget · target=zanfona (topic)

[en] U: forget that a guitar is a string instrument
     T: That's something I came with. Are you sure you want me to forget it?
                                           → asks · confirm   (teach.forget)
[en] U: yes
     T: Done: I no longer hold that a guitar is a string instrument.
                                           → answered · teach.forget · tombstone=C01
[en] U: is a guitar a musical instrument?
     T: Nothing I know says a guitar is a musical instrument.
                                           → unhandled · relations.is_a
     (the path through C01 is gone; nothing known, not "no")

Given: U1.
[en] U: forget everything I taught you
     T: That's 3 things. Forget them all?  → asks · confirm   (teach.forget)
[en] U: actually no
     T: Okay, I'll keep them.              → answered · teach.forget · nothing changed

Given: U3.
[en] U: forget "flip it"
     T: Forgotten: "flip it" no longer means tossing a coin.
                                           → answered · teach.forget · target=phrase "flip it"

[en] U: is a whale a fish?
     T: No: a whale is a mammal, not a fish.
                                           → answered · relations.is_a
[en] U: that's wrong, forget it
     T: That's something I came with: a whale is not a fish. Are you sure you
        want me to forget it?              → asks · confirm   (teach.forget)
     (the last answer used C41 and C40; the contradicting claim, C41, is the one named)
```

#### Edge cases

```text
[es] U: olvida que la guitarra es un instrumento musical
     T: Eso no me lo dijo nadie así: lo deduzco de que la guitarra es un
        instrumento de cuerda y los instrumentos de cuerda son instrumentos
        musicales. ¿Cuál olvido?           → asks · choice   (teach.forget)
[es] U: ninguna
     T: Vale, no olvido nada.              → answered · teach.forget · nothing changed

[es] U: olvida que la flauta es un instrumento de cuerda
     T: No tenía nada que dijera eso.      → answered · teach.forget · nothing changed

[es] U: olvida todo lo de la guitarra
     T: Lo que sé de la guitarra lo traigo de serie, y eso no lo olvido de
        golpe: dime qué cosa en concreto.  → asks · target   (teach.forget)
     (pack claims are never forgotten by topic)

[en] U: forget my name
     T: (preferences.forget answer)        → answered · preferences.forget

[es] U: olvídalo
     T: Vale.                              → answered · no intent
     (nothing open, no content: see conversation.md 3.5)

Given: the tombstone of C40 from «olvida que la ballena es un mamífero» above.
[es] U: aprende: la ballena es un mamífero
     T: Hecho: vuelvo a considerar que la ballena es un mamífero.
                                           → answered · teach.assert · removes tombstone of C40
     (re-teaching a forgotten pack claim lifts the tombstone instead of adding a duplicate)
```

### teach.list

**Recognises** — seeing what has been taught: «¿qué te he enseñado?», «¿qué
has aprendido de mí?», «dime lo que te he enseñado», «enséñame lo que has
aprendido» (here «enséñame» means "show me"), «¿qué te he enseñado sobre X?»,
«¿qué frases te he enseñado?», «¿qué te enseñé ayer / esta semana?»; "what
have I taught you?", "what have you learned from me?", "list what I've taught
you", "what did I teach you about X?", "what phrases have I taught you?",
"anything I taught you this week?".

**Slots**
- `topic` — `term`, optional: only claims whose subject or object is the
  concept or one of its kinds.
- `period` — `datetime`, optional: «hoy», «ayer», «esta semana», "today",
  "yesterday", "this week".
- `kind` — `choice` (`all` | `facts` | `phrases`), optional; default `all`.

**Answers** — the count, then the items, newest last, each in the
conversation language with its date: taught claims, overrides of pack
knowledge (marked), forgotten pack claims (marked), and taught phrases (with
what they do). At most 10 items, then how many more. Nothing to list is an
answer, not a failure (`answered`).

**Does not**
- «¿qué sabes de X?» → `relations.about`.
- «¿qué sabes de mí?» → `preferences.get`.
- «enséñame algo» (teach me something) → not this intent.

#### Examples

```text
Given: U1.
[es] U: ¿qué te he enseñado?
     T: Tres cosas: que la zanfona es un instrumento de cuerda (10 de marzo),
        que el theremin es un instrumento electrónico (11 de marzo) y que el
        calor provoca sequía (12 de marzo).
                                           → answered · teach.list · count=3

[es] U: ¿qué has aprendido de mí?
     T: Todavía no me has enseñado nada.   → answered · teach.list · count=0

Given: U2.
[es] U: dime lo que te he enseñado
     T: Cuatro cosas: que la zanfona es un instrumento de cuerda (10 de marzo),
        que el theremin es un instrumento electrónico (11 de marzo), que el
        calor provoca sequía (12 de marzo) y que el frío provoca resfriados
        (13 de marzo; corrige lo que traía de serie).
                                           → answered · teach.list · count=4

Given: U1.
[es] U: ¿qué te he enseñado sobre instrumentos?
     T: Dos cosas: que la zanfona es un instrumento de cuerda y que el
        theremin es un instrumento electrónico.
                                           → answered · teach.list · topic=instrumento musical · count=2
     (both objects are kinds of musical instrument)

Given: U3.
[es] U: ¿qué frases te he enseñado?
     T: Dos: «tira» para tirar un dado y «flip it» para echar una moneda al
        aire.                              → answered · teach.list · kind=phrases · count=2

Given: U1.
[es] U: ¿qué te he enseñado hoy?
     T: Hoy, nada. Lo último fue el 12 de marzo: que el calor provoca sequía.
                                           → answered · teach.list · period=2027-03-14 · count=0

Given: U1, then «olvida que la ballena es un mamífero» confirmed on 2027-03-14.
[es] U: enséñame lo que has aprendido
     T: Cuatro cosas: que la zanfona es un instrumento de cuerda (10 de
        marzo), que el theremin es un instrumento electrónico (11 de marzo),
        que el calor provoca sequía (12 de marzo) y, hoy, me pediste olvidar
        que la ballena es un mamífero, que traía de serie.
                                           → answered · teach.list · count=4
```

```text
Given: U1.
[en] U: what have I taught you?
     T: Three things: a zanfona is a string instrument (10 March), a theremin
        is an electronic instrument (11 March), and heat causes drought (12
        March).                            → answered · teach.list · count=3
     (items taught in Spanish are listed in English; "zanfona" keeps its only word)

[en] U: list what I've taught you
     T: You haven't taught me anything yet.
                                           → answered · teach.list · count=0

Given: U2.
[en] U: what have you learned from me?
     T: Four things: a zanfona is a string instrument (10 March), a theremin
        is an electronic instrument (11 March), heat causes drought (12
        March), and cold weather causes colds (13 March; this overrides what I
        came with).                        → answered · teach.list · count=4

Given: U1.
[en] U: what did I teach you about heat?
     T: One thing: heat causes drought (12 March).
                                           → answered · teach.list · topic=heat · count=1

Given: U3.
[en] U: what phrases have I taught you?
     T: Two: «tira» to roll a die, and "flip it" to toss a coin.
                                           → answered · teach.list · kind=phrases · count=2

Given: U1.
[en] U: anything I taught you this week?
     T: Three things, all this week: a zanfona is a string instrument, a
        theremin is an electronic instrument, and heat causes drought.
                                           → answered · teach.list · period=2027-03-08..14 · count=3
```

#### Edge cases

```text
Given: U1 plus 12 more taught claims.
[es] U: ¿qué te he enseñado?
     T: Quince cosas. Las diez últimas: … (diez elementos). Y cinco más.
                                           → answered · teach.list · count=15
     (at most ten items; the rest counted)

Given: U1.
[en] U: what did I teach you about whales?
     T: Nothing about whales.              → answered · teach.list · topic=whale · count=0

[es] U: ¿qué sabes de mí?
     T: (preferences.get answer)           → answered · preferences.get
```

### teach.phrase

**Recognises** — teaching that a phrase of the user's means a request Talos
already understands: «cuando diga «X», quiero decir «Y»», «cuando te diga X
me refiero a Y», «si te digo «X», quiero decir «Y»», «con «X» me refiero a
«Y»»; "when I say 'X' I mean 'Y'", "if I say X, I mean Y", "by 'X' I mean
'Y'". Quotes are optional but help; without them the split is at «quiero
decir» / «me refiero a» / "I mean".

**Slots**
- `phrase` — `term`, required.
- `meaning` — `term`, required. Asked if missing.

**Answers** — Talos understands `meaning` the way it would understand it as
a message (rules or a confident classifier result, with its slots). If it
resolves to an intent, the mapping phrase → intent + slots is stored in
`user.db`, and the reply says what Talos will do when it sees the phrase. From
then on, a message that **is** the phrase (after the normalisation of
[conversation.md](conversation.md) 2: case, accents, spaces, trailing
punctuation) is handled as that intent, in either conversation language, and
the reply is in the conversation's language. A message that merely contains
the phrase is not affected.

- `meaning` not understood → says so, nothing stored (`unhandled`).
- `meaning` resolves to `teach.forget`, `teach.phrase`, `preferences.forget`
  or anything else that deletes → refused (`unhandled`): a short phrase must
  not be able to erase things.
- `phrase` is already understood as something else (a built-in rule, or an
  earlier mapping) → asks whether to replace that meaning for this user.
- `meaning` is a word or a thing, not a request («me refiero a la guitarra»)
  → says it only links phrases to requests (`unhandled`); a synonym is taught
  with `teach.assert`.

**Does not**
- ««sinte» es otra forma de decir sintetizador» → `teach.assert` (synonym).
- ««zanfona» significa …» → `teach.assert` (meaning).
- «¿qué quiere decir «tira»?» → `define.word`.

#### Examples

```text
[es] U: cuando diga «tira», quiero decir «tira un dado»
     T: Vale: cuando digas «tira», tiraré un dado.
                                           → answered · teach.phrase · phrase=tira · meaning=random.dice
[es] U: tira
     T: Ha salido un 4.                    → answered · random.dice · via=phrase «tira»
     (the number is illustrative; see conversation.md 5.3)

[es] U: cuando te diga «la de Tokio» me refiero a «qué hora es en Tokio»
     T: Vale: cuando digas «la de Tokio», te diré la hora en Tokio.
                                           → answered · teach.phrase · phrase=la de Tokio · meaning=timezones.time_in place=Tokio

[es] U: si te digo «¿y hoy?», quiero decir «qué día es hoy»
     T: Vale: cuando digas «¿y hoy?», te diré la fecha de hoy.
                                           → answered · teach.phrase · phrase=¿y hoy? · meaning=clock.date

[es] U: con «sorpréndeme» me refiero a «dime un refrán»
     T: Vale: cuando digas «sorpréndeme», te diré un refrán.
                                           → answered · teach.phrase · phrase=sorpréndeme · meaning=fortune.tell

[es] U: cuando diga «la de seis cuerdas», quiero decir «qué es una guitarra»
     T: Vale: cuando digas «la de seis cuerdas», te diré qué es una guitarra.
                                           → answered · teach.phrase · phrase=la de seis cuerdas · meaning=define.word word=guitarra

Given: U3.
[es] U: Tira.
     T: Ha salido un 2.                    → answered · random.dice · via=phrase «tira»
     (case and trailing punctuation are ignored)
```

```text
[en] U: when I say 'flip it' I mean 'toss a coin'
     T: Okay: when you say "flip it", I'll toss a coin.
                                           → answered · teach.phrase · phrase=flip it · meaning=random.coin
[en] U: flip it
     T: Heads.                             → answered · random.coin · via=phrase "flip it"

[en] U: if I say "tokyo?", I mean "what time is it in Tokyo"
     T: Okay: when you say "tokyo?", I'll tell you the time in Tokyo.
                                           → answered · teach.phrase · phrase=tokyo? · meaning=timezones.time_in place=Tokyo

[en] U: by "today?" I mean "what's the date today"
     T: Okay: when you say "today?", I'll tell you today's date.
                                           → answered · teach.phrase · phrase=today? · meaning=clock.date

[en] U: when I say "hit me" I mean "roll a die"
     T: Okay: when you say "hit me", I'll roll a die.
                                           → answered · teach.phrase · phrase=hit me · meaning=random.dice

[en] U: when I say "wisdom" I mean "tell me a fortune"
     T: Okay: when you say "wisdom", I'll tell you a saying.
                                           → answered · teach.phrase · phrase=wisdom · meaning=fortune.tell

[en] U: when I say "six-string" I mean "what is a guitar"
     T: Okay: when you say "six-string", I'll tell you what a guitar is.
                                           → answered · teach.phrase · phrase=six-string · meaning=define.word word=guitar
```

#### Edge cases

```text
[es] U: cuando diga «abracadabra», quiero decir «haz magia»
     T: No sé qué hacer con «haz magia», así que no puedo asociarle nada.
                                           → unhandled · teach.phrase
     (the meaning must be a request Talos understands)

[es] U: cuando diga «limpia», quiero decir «olvida todo lo que te he enseñado»
     T: Eso no: no asocio frases a peticiones que borran cosas.
                                           → unhandled · teach.phrase
     (deleting must always be asked for in full)

[en] U: when I say "what time is it" I mean "roll a die"
     T: "What time is it" already means telling you the time. Do you want it
        to mean rolling a die instead?     → asks · confirm   (teach.phrase)
[en] U: no
     T: Okay, I'll leave it as it was.     → answered · teach.phrase · nothing stored

Given: U3.
[es] U: cuando diga «tira», quiero decir «echa una moneda»
     T: «Tira» ya significa tirar un dado. ¿Lo cambio por echar una moneda?
                                           → asks · confirm   (teach.phrase)
[es] U: sí
     T: Hecho: cuando digas «tira», echaré una moneda al aire.
                                           → answered · teach.phrase · replaces earlier mapping

[es] U: cuando digo «la de seis» me refiero a la guitarra
     T: Solo puedo asociar frases a peticiones, y «la guitarra» no es una
        petición.                          → unhandled · teach.phrase

[en] U: when I say "flip it"
     T: What do you mean by it?            → asks · meaning   (teach.phrase)

Given: U3.
[es] U: tira dos veces
     T: (handled as an ordinary message; the phrase is not part-matched)
                                           → per the normal pipeline

Given: U3; a conversation in Spanish.
[es] U: flip it
     T: Ha salido cara.                    → answered · random.coin · via=phrase "flip it"
     (a taught phrase works in any conversation; the reply stays in Spanish, conversation.md 1.2)
```

---

## translate

Words and stock phrases between Spanish and English, and nothing else. A word
is translated by finding the lexemes of the other language that share its
concept (KNOWLEDGE.md §1); failing that, by the word-pair lexicon
(KNOWLEDGE.md §7, fixture 1.8); a stock phrase by the table of 1.6. A word with
several senses gives one translation per sense unless the sense is known.
Translating free sentences needs a language model and Talos says so plainly
(ADR 0012); it never translates word by word, never invents a word for a
concept that has none in the target language, and translates to no third
language.

### translate.word

**Recognises** — «¿cómo se dice X en inglés/español?», «X en inglés»,
«traduce X», «tradúceme X», «pásame X al inglés», «¿qué es X en español?»,
«¿cómo se dice X?»; "how do you say X in Spanish?", "what's X in Spanish?",
"translate X", "Spanish for X?", "X in English", "how do I say X in
Spanish?".

**Slots**
- `term` — `term`, required.
- `target` — `language`, optional; default: the language the term is not in;
  for a word that exists in both, the language the conversation is not in.
- `sense` — `choice`, optional; set by 2.2 steps 1–3 only.

**Answers** — the target lexeme(s): rank 0 first, others as «también» /
"also". If the input was an inflected form and the target lexeme has a form
with the same features, that form. If the sense is not known from memory or
context, one translation per sense, each labelled by its class or gloss (no
question: the answer itself disambiguates). A stock phrase: its counterpart,
with variants by situation and a note when the match is approximate. A
concept with no lexeme in the target language, and no lexicon entry: says
there is no word for it (`unhandled`). A third language, or anything that is
neither a lexeme, a lexicon entry nor a stock phrase: says what Talos
translates (`unhandled`).

**The word-pair lexicon** is the fallback for single words, consulted only
when the concept store gives no word in the target language (the term is
unknown there, or its concept has no lexeme in the target language):

- The reply gives the lexicon's first candidate, and the second as «también» /
  "also" if there is one, and **says in words that it is less reliable**: it
  comes from a word-pair dictionary, not from what Talos knows about the
  word's meaning. The hedge is always there, whatever the rank, because the
  lexicon is noisy and has no senses. Status `answered`.
- An answer from the concept store never carries that hedge, and the lexicon
  is not consulted for a word the concept store translates.
- The lexicon is **never reasoned from**. It adds no concept, sense or claim:
  `define` and `relations` do not use it, a lexicon-only word stays an
  unknown word for them, and nothing is inferred from a pair (the lexicon
  saying «tamboril» → "drum" does not make a *tamboril* a percussion
  instrument). Its other uses in KNOWLEDGE.md §7, induced Spanish senses at
  pack-build time and a run-time bridge to English concepts, are conditional
  on measuring their precision and are not part of this specification.
- `relations.provenance` on such an answer names the lexicon as the source.

**Does not**
- «¿qué es bench?» (what a foreign word *is*) → `define.word` (2.1, step 3).
- «tradúceme esta frase: …», "translate this email" → `unhandled` (this
  intent, refusing).
- «¿cómo se escribe guitarra?», "how do you pronounce guitar?": `unhandled`,
  no intent.

#### Examples

```text
[es] U: ¿cómo se dice guitarra en inglés?
     T: «Guitar».                          → answered · translate.word · term=guitarra · target=en

[es] U: ¿cómo se dice «instrumento de cuerda» en inglés?
     T: «String instrument»; también «stringed instrument».
                                           → answered · translate.word · term=instrumento de cuerda · target=en

[es] U: traduce violín
     T: «Violin»; también «fiddle».        → answered · translate.word · term=violín · target=en

[es] U: guitarras en inglés
     T: «Guitars».                         → answered · translate.word · term=guitarras · target=en
     (plural kept: the target lexeme has a plural form)

[es] U: ¿cómo se dice banco en inglés?
     T: Depende: «bank» si es la entidad financiera, «bench» si es para
        sentarse, y «school» o «shoal» si es de peces.
                                           → answered · translate.word · term=banco · target=en
     (no sense known: one translation per sense, no question)

[es] U: ¿cómo se dice banco en inglés, el de sentarse?
     T: «Bench».                           → answered · translate.word · term=banco · sense=core:bench

[es] U: ¿cómo se dice cuerda en inglés?
     T: «Rope»; si es la de un instrumento, «string».
                                           → answered · translate.word · term=cuerda · target=en

[es] U: ¿cómo se dice «de nada» en inglés?
     T: «You're welcome».                  → answered · translate.word · term=de nada (stock phrase)
[es] U: ¿y «buen provecho»?
     T: No hay una expresión fija en inglés; lo más parecido es «enjoy your
        meal».                             → answered · translate.word · term=buen provecho (stock phrase, approximate)
     (the follow-up keeps the target, 2.8)

[es] U: ¿cómo se dice «bench» en español?
     T: «Banco» (el de sentarse).          → answered · translate.word · term=bench · target=es
```

```text
[en] U: how do you say guitar in Spanish?
     T: «Guitarra».                        → answered · translate.word · term=guitar · target=es

[en] U: what's "string instrument" in Spanish?
     T: «Instrumento de cuerda».           → answered · translate.word · term=string instrument · target=es

[en] U: translate fiddle
     T: «Violín».                          → answered · translate.word · term=fiddle · target=es

[en] U: Spanish for drums?
     T: «Tambores».                        → answered · translate.word · term=drums · target=es

[en] U: how do you say bank in Spanish?
     T: «Banco» for the financial institution; «orilla» or «ribera» for the
        side of a river.                   → answered · translate.word · term=bank · target=es

[en] U: what's "school" in Spanish?
     T: «Escuela» or «colegio»; a school of fish is a «banco».
                                           → answered · translate.word · term=school · target=es

[en] U: what's "cold" in Spanish?
     T: «Frío» for the temperature; the illness is «resfriado» or «catarro».
                                           → answered · translate.word · term=cold · target=es

[en] U: how do I say "nice to meet you" in Spanish?
     T: «Encantado» if you're a man, «encantada» if you're a woman; also
        «encantado de conocerte».          → answered · translate.word · term=nice to meet you (stock phrase)

[en] U: how do you say "good evening" in Spanish?
     T: «Buenas tardes» while it's still light, «buenas noches» after dark.
                                           → answered · translate.word · term=good evening (stock phrase)
```

From the word-pair lexicon (1.8):

```text
[es] U: ¿cómo se dice pandereta en inglés?
     T: Seguramente «tambourine», pero lo saco de un diccionario de pares de
        palabras, menos fiable que lo demás que sé.
                                           → answered · translate.word · term=pandereta · target=en
     (unknown in the concept store; the lexicon answers, always with the hedge)

[es] U: gaita en inglés
     T: Seguramente «bagpipes»; también «bagpipe». Lo saco de un diccionario
        de pares de palabras, menos fiable que lo demás.
                                           → answered · translate.word · term=gaita · target=en

[es] U: ¿cómo se dice tamboril en inglés?
     T: Seguramente «drum»; también «tabor». Lo saco de un diccionario de pares
        de palabras, menos fiable que lo demás.
                                           → answered · translate.word · term=tamboril · target=en
[es] U: ¿el tamboril es un instrumento de percusión?
     T: No conozco la palabra «tamboril».  → unhandled · relations.is_a · subject=tamboril
     (the lexicon is never reasoned from: "drum" being a percussion instrument
      says nothing about the tamboril)

[es] U: ¿cómo se dice guitarra en inglés?
     T: «Guitar».                          → answered · translate.word · term=guitarra · target=en
     (the concept store answers, with no hedge; the lexicon entry is not consulted)

[en] U: how do you say tambourine in Spanish?
     T: Probably «pandereta», but that comes from a word-pair dictionary, which
        is less reliable than the rest of what I know.
                                           → answered · translate.word · term=tambourine · target=es

[en] U: what's "accordion" in Spanish?
     T: Probably «acordeón»; it comes from a word-pair dictionary, so it's less
        reliable than the rest.            → answered · translate.word · term=accordion · target=es

[en] U: translate cord
     T: Probably «cuerda»; also «cordón». That comes from a word-pair
        dictionary, which is less reliable than the rest.
                                           → answered · translate.word · term=cord · target=es
[en] U: where did you get that?
     T: From the word-pair dictionary, not from anything I know about what
        "cord" means.                      → answered · relations.provenance · claim=last
[en] U: is a cord part of a guitar?
     T: I don't know the word "cord".      → unhandled · relations.part_of · part=cord · whole=guitar
     (the pair cord → cuerda is not a sense: Talos does not borrow core:string from it)
```

#### Edge cases

```text
[es] U: ¿cómo se dice bandurria en inglés?
     T: No tengo una palabra en inglés para «bandurria».
                                           → unhandled · translate.word · term=bandurria · target=en
     (known concept, no English lexeme: nothing invented)

[en] U: translate dulcimer
     T: I don't have a Spanish word for "dulcimer".
                                           → unhandled · translate.word · term=dulcimer · target=es

Given: U1.
[es] U: ¿cómo se dice zanfona en inglés?
     T: No tengo una palabra en inglés para «zanfona».
                                           → unhandled · translate.word · term=zanfona
[es] U: aprende: zanfona en inglés se dice hurdy-gurdy
     T: Aprendido: «zanfona» en inglés es «hurdy-gurdy».
                                           → answered · teach.assert
[es] U: ¿cómo se dice zanfona en inglés?
     T: «Hurdy-gurdy».                     → answered · translate.word · term=zanfona

[es] U: tradúceme «mañana me compro una guitarra»
     T: Solo traduzco palabras sueltas y frases hechas; frases enteras, no.
                                           → unhandled · translate.word
     (every word is known; Talos still does not translate word by word)

[en] U: translate "my brother plays the violin"
     T: I only translate single words and set phrases, not sentences.
                                           → unhandled · translate.word

[es] U: ¿cómo se dice «guitarra roja» en inglés?
     T: Solo traduzco palabras sueltas y frases hechas; frases enteras, no.
                                           → unhandled · translate.word
     (two known words that are not one lexeme are a free phrase)

[es] U: ¿cómo se dice guitarra en francés?
     T: Solo traduzco entre español e inglés.
                                           → unhandled · translate.word · target=francés

[en] U: what does "banco" mean in English?
     T: «Banco» is a bank (the financial institution), a bench, or a school of
        fish.                              → answered · translate.word · term=banco · target=en

[es] U: ¿cómo se dice guitarra?
     T: En inglés, «guitar».               → answered · translate.word · term=guitarra · target=en
     (no target: the other language)

[es] U: ¿cómo se dice virus en inglés?
     T: Igual: «virus».                    → answered · translate.word · term=virus · target=en

[es] U: ¿cómo se dice «salud» en inglés?
     T: Si alguien estornuda, «bless you»; para brindar, «cheers».
                                           → answered · translate.word · term=salud (stock phrase)
     (a stock phrase of 1.6: one counterpart per situation)
```

---

## Decisions taken in this specification

The design documents leave these behaviours open. Each was decided here in
the way most consistent with the ADRs; the owner should review them.

1. **Status of absence.** Answers that rest on the absence of knowledge
   (unknown word, nothing known, beyond depth, no word in the other language)
   are `unhandled` with the intent kept, so a host can pass the turn on
   (DESIGN §3, ADR 0009). Contradicted negatives are `answered`. An empty
   `teach.list` and "nothing to forget" are `answered`, because they are
   complete answers about the user's own store.
2. **Three kinds of "no"**, including "beyond depth" as distinct from "nothing
   known", reported when the bounded search stopped with links unexplored
   from both ends (subject upward, class downward).
3. **Confidence bands**: plain ≥ 0.85, hedged 0.60–0.85, weak 0.30–0.60, not
   affirmed below 0.30. Wording is epistemic («por lo que sé», "possibly"),
   never frequency words. Numbers only on request, as rounded percentages.
   **Ruled (ruling 3):** accepted as initial values that may be tuned.
4. **Decay of 0.99 per link after the first**, and **taught confidence
   0.95**: enough that six links of pack claims or one taught claim plus a few
   pack links stay plain. **Ruled (ruling 3):** initial values, may be tuned.
5. **Path wording**: full path up to three links; beyond that first link,
   last link and the step count; full path on `relations.provenance`.
6. **Taught knowledge is always attributed** in answers («según me
   enseñaste»); pack knowledge is not attributed unless asked or in conflict.
7. **Sense selection order**: remembered choice, context in the turn, context
   of the previous three turns, structural fit (only one sense can answer the
   relation asked), strict rank winner, then ask. A tie in rank 0 means ask.
8. **Sense memory** lasts for the conversation, is shared by all four skills,
   is replaced by new explicit context, and is never written to disk.
9. **Translation does not ask** on ambiguity: it lists one translation per
   sense, since the list is itself the disambiguation. Define, relations and
   teach do ask on a tie.
10. **Cross-language lookup**: a word not found in the conversation language
    is looked up in the other one; «¿qué es bench?» is `define.word`, answered
    with the conversation-language gloss and word.
11. **Missing gloss**: Talos says it has no definition in this language and
    falls back to the word's `is_a`, then `part_of` claim; the other
    language's gloss only when explicitly asked, quoted and labelled. The
    status is `unhandled · define.word`, since no definition was given.
    **Ruled (ruling 8).**
12. **Near misses**: one known lemma at one edit is asked as a yes/no; zero or
    several candidates, no suggestion. Candidates are sought in the
    conversation language first.
13. **Ambiguous Spanish causal questions** («¿qué provoca X?») are answered in
    both directions, causes first.
14. **Lists**: kinds and parts list one level by default, the closure on
    «todos»/"all" (cap 15); `relations.about` gives up to five items (a list
    counts as one) and counts the rest; `teach.list` caps at 10.
15. **Teaching cues and shapes** are closed lists (Teaching forms above).
    «recuerda que»/"remember that" teaches only when the statement is about
    something other than the user; otherwise it is `preferences.set`.
    Opinions, adjectives, quantities and time-limited facts are unsupported.
16. **Ordinary statements** without a cue go to the reflector, are never
    stored, never used as context, and Talos gives no hint that they could be
    taught (no unsolicited suggestions).
17. **Unknown subject creates a word without asking; unknown object is
    asked** (near-miss or "learn it as new?"). New words are stored as typed,
    without lemma guessing.
18. **Re-teaching what is already known** (direct or derived, any source)
    stores nothing; there is no user corroboration of pack claims.
19. **Contradictions**: pack → ask to override; user → ask to replace;
    derived → ask which link, then override it and store the statement; `is_a`
    cycle → refuse without asking. A negated claim about a subclass under a
    positive class claim is an exception, stored without asking.
20. **Forgetting**: a user claim is retracted at once; a pack claim needs
    confirmation and leaves a tombstone; topic-wide and "all" forgetting cover
    only taught knowledge and confirm when more than one item; a user-created
    word with nothing left is removed; re-teaching a forgotten pack claim
    lifts its tombstone.
21. **Taught phrases** match the whole message after normalisation, work in
    either conversation language, may not map to deleting intents or to
    `teach.phrase`, and ask before shadowing an existing meaning.
22. **Time**: lists filter by validity at the reference time;
    `relations.about` also lists expired claims marked as past; taught claims
    carry no validity interval.
23. **Free-phrase translation** is refused even when every word is known;
    an inflected input keeps its features in the target when the form exists.
24. **Word-pair lexicon** (KNOWLEDGE.md §7, role 1 only): consulted for
    `translate.word` only after the concept store gives no word; up to two
    candidates, always with a spoken hedge that the source is less reliable;
    `answered`. Never used by `define` or `relations` and never reasoned from.
    **Ruled (ruling 9).**
25. **Weak-band Spanish wording** avoids «no estoy seguro» (gendered); it says
    «no lo sé con certeza».
