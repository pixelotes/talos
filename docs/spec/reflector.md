# Reflector: the fallback, and `deflected` vs `unhandled`

The reflector is the last stage of the pipeline ([DESIGN.md](../DESIGN.md)
section 4, stage 7). It runs only when no skill answered and no open question
took the message (distress, S29, is the exception: it is checked before the
open question, conversation.md 3.9). It is a pattern-reflection script in the manner of ELIZA
(Weizenbaum, 1966): it recognises the *shape* of a sentence and hands the
conversation back, sometimes echoing part of what the user wrote with the
persons swapped. The scripts are original, one per language, and the Spanish
one is written as Spanish ([ADR 0009](../adr/0009-reflector-fallback.md),
[ADR 0011](../adr/0011-bilingual-by-construction.md)).

Format and global rules are in [README.md](README.md). Refusal wording and the
treatment of acknowledgements, bare "sí"/"no", hedges, gibberish and other
languages are in [conversation.md](conversation.md); this file follows them and
says where the reflector stops.

---

## 1. Purpose and design rules

Stand-alone, Talos fails to recognise much of what people type. Answering all
of it with "I didn't understand that" makes it feel broken where it could
simply be polite. Behind a host, the same turns must be easy to pass on. The
reflector serves both by keeping the conversation moving **and** labelling
every turn honestly:

| Status | Produced by | Meaning for a host |
|---|---|---|
| `answered` | a skill | a real answer |
| `deflected` | the reflector, on a recognised sentence shape | Talos kept the conversation going without answering anything; show it, or pass the turn on |
| `unhandled` | the reflector's guards, refusals, or the bare catch-all | nothing useful was produced; pass the turn on |

Rules, in order of importance:

1. **Recognise the form, never imply understanding of the content.** A reply
   may refer to *what the user said* ("you think that…", "your sister…") but
   never to *what it means* ("that sounds hard", "Madrid is lovely"). The
   **substitution test** decides: if the echoed words were replaced by any
   other words of the same grammatical kind, the reply must still make sense
   and still be true. "¿Qué tiene el flamenco que te mola tanto?" passes
   (works for *madrugar*, *el reguetón*, *los lunes*); "¿Has probado a
   bailarlo?" fails.
2. **More sentence shapes, not more therapy.** Coverage grows by recognising
   more kinds of sentence (statements, plans, comparisons, remarks about
   Talos), not by adding prompts that probe the user.
3. **Never ask how the user feels**, never interpret their emotions, never
   reassure, advise, praise, judge or diagnose. No reply comments on the user
   or on people they mention.
4. **Factual-looking questions are not reflected.** A question about the world
   that no skill answered gets an honest "I don't know", status `unhandled`
   (section 4). Reflecting it would pretend the question had been heard.
5. **Talos has no opinions, tastes, feelings, memories or experiences**, and
   no reply claims or implies any. It says "I don't" plainly when asked.
6. **No false promises.** Nothing is remembered by the reflector, so no reply
   says "I'll remember", "noted", "lo tendré en cuenta", "lo apunto".
7. **Grammar first.** An echo is used only when it comes out grammatical
   (section 3). Otherwise the shape's neutral variant is used; the status does
   not change.
8. **Short.** One or two short sentences; at most **one** question, which
   hands the turn back about what the user said ("¿Por qué lo dices?",
   "And then?") and never proposes anything.

What the reflector is **not**: a way to answer, a source of facts, a
personality. It never produces `answered`.

---

## 2. Sentence shapes

### 2.1 How a shape is chosen

- The reflector runs only after rules, classifier and pending dialog have all
  passed on the message, except for distress (S29), which is checked
  before the pending dialog (conversation.md 3.9). Anything a skill or [conversation.md](conversation.md)
  claims is not seen here: greetings, thanks, goodbyes, "how are you",
  questions about what Talos is (`identity.*`) or can do (`help.*`), bare
  acknowledgements ("vale", "ok", "genial", 👍), bare "sí"/"no"/"yes"/"no"
  (conversation.md 3.8), hedges while a question is open (3.7), choices
  with a verb of choosing ("elige: X o Y") that `random.pick` takes. A bare
  "¿X o Y?" with no such verb asks for Talos's preference and is S16.
- **Guards run first** (section 4). A message that fails a guard is
  `unhandled` and no shape is tried. Distress (S29) is checked before
  everything, including the guards.
- Among shapes, the more specific wins. The ranking, highest first:
  1. distress (S29);
  2. requests Talos cannot meet (S27, S28);
  3. remarks and questions aimed at Talos (S19 insult, S18 compliment,
     S26 "you don't understand", S17, S16, S15, S25);
  4. conversational acts (S20 apology, S21 agreement, S22 disagreement,
     S23 exclamation, S24 backchannel);
  5. statements about the user's world, in this order: S9 hypothetical,
     S8 comparison, S7 like/dislike, S3 plan, S4 wish, S12 complaint,
     S13 self-directed why, S10 generalisation, S1 opinion, S14 third
     party, S11 uncertainty, S6 past event, S5 self-description, S2
     assertion.
- "creo que eres un inútil" is an insult, not an opinion: aimed-at-Talos
  outranks the user's framing.
- A message with several sentences: every sentence is tried; the highest
  ranked shape found anywhere wins; on a tie, the last sentence wins.
- If no shape matches and no guard fired, the **bare catch-all** applies:
  status `unhandled`, reply "No te he entendido." / "I didn't understand
  that." (and variants, section 6).

### 2.2 Outcome lines in this file

The shape id is shown where other files show an intent:
`→ deflected · opinion`. Shape ids are **not** intents: the outcome's
`intent` field is empty for every reflector turn, and the shape id travels in
the trace. Unhandled turns show the reason the same way, also trace-only:
`→ unhandled · catch-all`, `→ unhandled · unknown-fact`.

Each shape lists **Identifies** (what makes a sentence this shape), **Reply**
(what kind of thing is said), **Status**, and examples. Reply wording is
illustrative; the kind of reply and the status are normative.

---

### S1 · Opinion (`opinion`)

**Identifies** — the user presents a view as their own: a verb of belief or
judgement in the first person followed by a clause ("creo que", "me parece
que", "opino que", "para mí", "a mi modo de ver", "yo diría que"; "I think",
"I reckon", "in my view", "I'd say", "I feel that" + clause).
**Reply** — restates it as the user's view (echo), or says it is the user's
view and Talos has none; may ask why they say so. Never agrees or disagrees.
**Status** — `deflected`.

```text
[es] U: creo que el teletrabajo ha venido para quedarse
     T: ¿Crees que el teletrabajo ha venido para quedarse? ¿Por qué lo dices?
                                               → deflected · opinion
[es] U: para mí la tortilla tiene que llevar cebolla
     T: Esa es tu postura. Yo no tengo opinión en eso.
                                               → deflected · opinion
[es] U: a mi modo de ver la reunión de hoy sobraba
     T: Para ti la reunión de hoy sobraba.     → deflected · opinion

[en] U: I reckon the new manager won't last a year
     T: You reckon the new manager won't last a year? What makes you say so?
                                               → deflected · opinion
[en] U: in my opinion jazz is overrated
     T: That's your view; I don't hold one.    → deflected · opinion
```

### S2 · Assertion about the world (`assertion`)

**Identifies** — a declarative sentence about things or people in general,
with no first or second person and no question ("el agua del grifo de aquí
sabe a cloro", "cats can't taste sweet things"). Phrasings that teach
("aprende que…", "remember that…") belong to `teach.assert`.
**Reply** — neither confirms nor denies; may ask why the user brings it up.
**Status** — `deflected`.

```text
[es] U: en Cádiz se come el mejor pescado de España
     T: Puede ser. Yo eso ni lo afirmo ni lo niego.
                                               → deflected · assertion
[es] U: los lunes el tráfico es peor
     T: ¿Ah, sí? ¿Y por qué lo sacas?          → deflected · assertion

[en] U: cats can't taste sweet things
     T: Could be. I can't vouch for it either way.
                                               → deflected · assertion
[en] U: the trains are always worse in autumn
     T: Is that so? Why bring it up?           → deflected · assertion
```

(The second English example also contains "always"; S10 outranks S2, so a
script may equally answer "Always?". Both are `deflected`.)

### S3 · Plan or intention (`plan`)

**Identifies** — the user states what they are going to do: "voy a" +
infinitive, "pienso" / "tengo pensado" / "me planteo" + infinitive, a
first-person present or future with a time ahead ("mañana me cojo el día",
"el verano que viene me mudo"); "I'm going to", "I'm planning to", "I'm
thinking of" + -ing, "I'll", "gonna".
**Reply** — echoes the plan as a question or a plain restatement; may ask
"¿y eso?"/"what's behind that?". Never calls it a good or bad idea.
**Status** — `deflected`.

```text
[es] U: el año que viene voy a dejar el curro
     T: ¿Vas a dejar el curro? ¿Y eso?         → deflected · plan
[es] U: mañana me cojo el día libre
     T: Mañana te coges el día libre, entonces.
                                               → deflected · plan
[es] U: estoy pensando en apuntarme a la EOI
     T: Así que te lo estás planteando.        → deflected · plan
     (neutral variant: "apuntarme" has an enclitic, see 3.1)

[en] U: I'm going to repaint the kitchen this weekend
     T: You're going to repaint the kitchen this weekend?
                                               → deflected · plan
[en] U: I'm thinking of learning German
     T: Learning German, then. What's behind that?
                                               → deflected · plan
```

### S4 · Wish (`wish`)

**Identifies** — a desire about the user's own world: "quiero" + infinitive
or noun, "me gustaría", "ojalá", "tengo ganas de", "me muero de ganas de";
"I want", "I'd like", "I'd love", "I wish", "I can't wait to", "I'm dying
to". A wish whose object is *Talos doing something* ("quiero que me
escribas…") is a request and goes to the guards or S28.
**Reply** — echo, optionally with "¿y qué harías?"/"say more"; never judges
whether it is realistic.
**Status** — `deflected`.

```text
[es] U: ojalá me tocara la lotería
     T: ¿Ojalá te tocara? ¿Y qué harías?       → deflected · wish
[es] U: me gustaría tener un piso con terraza
     T: Te gustaría tener un piso con terraza. Queda dicho.
                                               → deflected · wish
[es] U: tengo unas ganas de que llegue agosto…
     T: Ganas de que llegue agosto. ¿Y eso?    → deflected · wish

[en] U: I wish I could take a year off
     T: You wish you could take a year off? What would you do with it?
                                               → deflected · wish
[en] U: I'm dying to see that film
     T: You're dying to see that film? Say more.
                                               → deflected · wish
     (an idiom, not distress: see S29)
```

### S5 · Self-description or state (`self`)

**Identifies** — "soy" / "estoy" / "me siento" / "me encuentro" + a
description; "I'm", "I am", "I feel" + adjective or noun. Self-introductions
that the `preferences` skill takes ("me llamo", "vivo en") never get here.
**Reply** — for a neutral description, a plain echo ("Profe de instituto,
entonces."). For a state that sounds unpleasant (tired, sad, ill, fed up), a
plain acknowledgement that Talos can do little about it, with **no echo and
no question**: no probing, no comfort, no advice.
**Status** — `deflected`.

```text
[es] U: soy profe de instituto
     T: Profe de instituto, entonces.          → deflected · self
[es] U: estoy agotada
     T: Vaya. Poco puedo hacer yo con eso, pero aquí sigo.
                                               → deflected · self
[es] U: me encuentro un poco pachucho
     T: Vaya. Eso no está en mi mano, pero aquí sigo.
                                               → deflected · self

[en] U: I'm a night-shift nurse
     T: A night-shift nurse, then.             → deflected · self
[en] U: I'm exhausted
     T: Ah. Not much I can do about that, but I'm still here.
                                               → deflected · self
```

### S6 · Past event (`past`)

**Identifies** — the user tells something that happened to them: a
first-person past tense ("ayer estuve", "el finde me quedé", "he ido",
"acabo de"); "I went", "I've just", "last night I".
**Reply** — echo, then "¿Y luego?"/"And then?" or similar.
**Status** — `deflected`.

```text
[es] U: ayer estuve en el Prado
     T: Ayer estuviste en el Prado. ¿Y luego?  → deflected · past
[es] U: el finde me quedé sin batería en pleno monte
     T: ¿Te quedaste sin batería en pleno monte? ¿Y cómo acabó la cosa?
                                               → deflected · past

[en] U: I went to a gig last night
     T: You went to a gig last night? And then?
                                               → deflected · past
[en] U: I've just moved house
     T: You've just moved house. How did that come about?
                                               → deflected · past
```

### S7 · Like or dislike (`taste`)

**Identifies** — "me gusta(n)", "me encanta", "me mola", "me chifla", "odio",
"no soporto", "no aguanto", "me da asco"; "I like", "I love", "I'm into",
"I hate", "I can't stand".
**Reply** — echo, or "¿Qué tiene X…?" built from the user's own words;
never shares the taste ("a mí también") or opposes it.
**Status** — `deflected`.

```text
[es] U: me mola un montón el flamenco
     T: ¿Qué tiene el flamenco que te mola tanto?
                                               → deflected · taste
[es] U: no soporto a la gente que habla alto en el tren
     T: No soportas a la gente que habla alto en el tren. Queda dicho.
                                               → deflected · taste

[en] U: I love rainy Sundays
     T: What is it about rainy Sundays?        → deflected · taste
[en] U: I can't stand coriander
     T: You can't stand coriander? Since when? → deflected · taste
```

### S8 · Comparison or preference (`compare`)

**Identifies** — two things set against each other by the user: "prefiero X
a Y", "X es mejor/peor que Y", "me quedo con X", "antes X que Y"; "I prefer X
to Y", "X is better than Y", "I'd take X over Y". A question asking Talos to
choose is `random.pick` when it has a verb of choosing ("elige: X o Y") and
S16 otherwise ("¿X o Y?", "¿cuál es mejor, X o Y?"), not this.
**Reply** — restates the preference as the user's; Talos takes no side.
**Status** — `deflected`.

```text
[es] U: prefiero la sierra a la playa
     T: La sierra antes que la playa. Yo ahí no me meto.
                                               → deflected · compare
[es] U: el metro es mil veces mejor que el bus
     T: Mejor el metro que el bus, según tú. ¿Por qué?
                                               → deflected · compare

[en] U: I prefer tea to coffee
     T: Tea over coffee, then. I don't take sides.
                                               → deflected · compare
[en] U: vinyl sounds better than streaming
     T: Better than streaming, you say? What makes the difference?
                                               → deflected · compare
```

### S9 · Hypothetical (`hypothetical`)

**Identifies** — a supposition: "si yo fuera/tuviera/pudiera…", "¿y si…?",
"imagínate que", "suponte que"; "if I were/had/won", "what if", "suppose",
"imagine". A conditional about measurable facts ("si cuesta 40 € con el 20 %
de descuento…") belongs to `calc`.
**Reply** — Talos does not speculate; it echoes the user's supposition or
turns the question back ("¿Tú qué crees?").
**Status** — `deflected`.

```text
[es] U: si yo fuera rico me compraría una casa en Asturias
     T: Si fueras rico, una casa en Asturias. ¿Y qué más?
                                               → deflected · hypothetical
[es] U: ¿y si mañana no sale el sol?
     T: Imaginar no es lo mío. ¿Tú qué crees que pasaría?
                                               → deflected · hypothetical

[en] U: if I won the lottery I'd quit tomorrow
     T: If you won, you'd quit tomorrow? What would come after?
                                               → deflected · hypothetical
[en] U: what if cats could talk?
     T: I don't do what-ifs. What do you think would happen?
                                               → deflected · hypothetical
```

### S10 · Generalisation (`generalisation`)

**Identifies** — an absolute word in a statement: "siempre", "nunca",
"jamás", "todo el mundo", "nadie", "todos", "nada"; "always", "never",
"everyone", "nobody", "nothing", "every time".
**Reply** — picks up the absolute word itself as a short question
("¿Siempre?", "Nobody at all?"). It never argues the generalisation.
**Status** — `deflected`.

```text
[es] U: nadie me hace caso en las reuniones
     T: ¿Nadie? ¿Ni una sola persona?          → deflected · generalisation
[es] U: siempre llego tarde a todo
     T: ¿Siempre? Alguna excepción habrá.      → deflected · generalisation

[en] U: nobody reads the documentation
     T: Nobody at all?                         → deflected · generalisation
[en] U: everyone's glued to their phone these days
     T: Everyone? That's a lot of people.      → deflected · generalisation
```

### S11 · Uncertainty (`uncertain`)

**Identifies** — the user hedges about themselves: "no sé", "ni idea", "a lo
mejor", "igual", "quizá", "no lo tengo claro"; "I don't know", "dunno",
"maybe", "not sure", "I haven't decided". "No sé" + an indirect factual
question ("no sé cuál es la capital de Perú") is a factual question and goes
to the guards.
**Reply** — accepts it without pressing: "Vale, lo dejamos en el aire.",
"Fair enough."; no reassurance.
**Status** — `deflected`.

```text
[es] U: no sé, igual sí
     T: Igual sí. Lo dejamos en el aire.       → deflected · uncertain
[es] U: a lo mejor me apunto a clases de baile
     T: A lo mejor. Ya dirás.                  → deflected · uncertain

[en] U: dunno really
     T: Fair enough.                           → deflected · uncertain
[en] U: maybe I'll call her tomorrow
     T: Maybe, then. Your call.                → deflected · uncertain
```

### S12 · Complaint (`complaint`)

**Identifies** — displeasure at a situation: "estoy harto/a de", "estoy hasta
las narices de", "me fastidia", "me revienta", "qué rollo", "vaya tela",
"no hay derecho"; "I'm sick of", "fed up with", "I hate it when", "ugh",
"so annoying".
**Reply** — a short commiserating noise mirrored from the user's register
("Vaya.", "That bad?"), optionally with the echo. Never "I understand",
never a solution, never a judgement of who is to blame.
**Status** — `deflected`.

```text
[es] U: estoy hasta las narices del ruido de las obras
     T: Hasta las narices del ruido de las obras. Vaya tela.
                                               → deflected · complaint
[es] U: qué rollo de lunes
     T: Vaya. ¿Tan malo está siendo?           → deflected · complaint

[en] U: I'm sick of these trains being late
     T: You're sick of these trains being late. That bad?
                                               → deflected · complaint
[en] U: ugh, Mondays
     T: That bad?                              → deflected · complaint
```

### S13 · Self-directed "why" (`why-me`)

**Identifies** — a why-question whose subject is the user and their habits or
luck: "¿por qué siempre me toca a mí…?", "¿por qué no consigo…?", "¿por
qué me lío tanto…?"; "why do I always…", "why can't I ever…". It is a
question nobody outside the user could answer, so it is not factual.
Questions seeking a diagnosis or a fix ("¿por qué me duele la rodilla?",
"why does my laptop overheat?") are factual or advice: guards.
**Reply** — Talos does not know; it turns the question back.
**Status** — `deflected`.

```text
[es] U: ¿por qué siempre me toca a mí fregar?
     T: ¿Por qué siempre te toca a ti? Eso no lo sé yo. ¿Tú qué crees?
                                               → deflected · why-me
[es] U: ¿por qué me lío tanto con las fechas?
     T: No tengo respuesta para eso. ¿Tienes alguna sospecha?
                                               → deflected · why-me

[en] U: why do I always leave things to the last minute?
     T: I couldn't tell you. What's your own guess?
                                               → deflected · why-me
[en] U: why can't I ever say no to people
     T: Why can't you, do you think?           → deflected · why-me
```

### S14 · Third party (`third-party`)

**Identifies** — the user tells about someone in their life: a possessive or
relation noun as subject ("mi madre", "mi jefe", "mi pareja", "una amiga
mía", "mi compi de piso"; "my mum", "my boss", "my partner", "a mate of
mine") with any verb.
**Reply** — echo with the possessive reflected, then a neutral hand-back.
Never an opinion about that person, never a guess at their motives.
**Status** — `deflected`.

```text
[es] U: mi hermana se ha comprado una moto
     T: ¿Tu hermana se ha comprado una moto? ¿Y eso?
                                               → deflected · third-party
[es] U: mi jefe dice que tengo que venir el sábado
     T: Tu jefe dice que tienes que venir el sábado.
                                               → deflected · third-party

[en] U: my sister's moving to Leeds
     T: Your sister's moving to Leeds? And what do you make of that?
                                               → deflected · third-party
[en] U: my boss wants the report by Friday
     T: Your boss wants the report by Friday.  → deflected · third-party
```

### S15 · Statement about Talos (`about-talos`)

**Identifies** — a description of Talos that is neither praise nor insult:
"eres muy formal", "hablas como un contestador", "pareces un robot de los
noventa"; "you're very literal", "you sound like a phone menu". Statements
about what Talos *is* ("eres una IA", "you're just a chatbot") are
`identity.nature`.
**Reply** — neither confirms nor protests; may add one true fact about how
Talos works, or ask what makes the user say so. **Never echoed in the first
person** (see 3.1).
**Status** — `deflected`.

```text
[es] U: hablas como un contestador automático
     T: Es posible. Contesto con frases hechas, eso es verdad.
                                               → deflected · about-talos
[es] U: eres muy formal, ¿no?
     T: Puede ser. ¿Qué te hace decirlo?       → deflected · about-talos

[en] U: you sound like a phone menu
     T: Fair. I work from set phrases, so that's not far off.
                                               → deflected · about-talos
[en] U: you're very literal
     T: Could be. What makes you say so?       → deflected · about-talos
```

### S16 · Question about Talos's opinions or tastes (`ask-opinion`)

**Identifies** — the user asks what Talos thinks, likes or would choose:
"¿qué opinas de…?", "¿te gusta…?", "¿cuál es tu … favorito/a?", "¿tú qué
harías?", "¿con cuál te quedas?"; "what do you think of", "do you like",
"what's your favourite", "what would you do".
**Reply** — Talos has no opinions or tastes and says so; may return the
question ("¿Y tú?"). It never picks a side to seem friendly.
**Status** — `deflected`.

```text
[es] U: ¿qué opinas del reguetón?
     T: No tengo opiniones, ni de eso ni de nada. ¿Tú qué opinas?
                                               → deflected · ask-opinion
[es] U: ¿cuál es tu peli favorita?
     T: No tengo gustos de ningún tipo. ¿Y tú?
                                               → deflected · ask-opinion
     (not "No tengo favoritas": see 3.2 on agreement)

[en] U: do you like jazz?
     T: I don't have likes or dislikes. Do you?
                                               → deflected · ask-opinion
[en] U: what would you do in my place?
     T: I wouldn't do anything; I don't make choices. What are you leaning towards?
                                               → deflected · ask-opinion
```

### S17 · Question about Talos's experience or inner life (`ask-experience`)

**Identifies** — questions that presuppose Talos lives a life: "¿te
aburres?", "¿duermes?", "¿sueñas?", "¿tienes amigos?", "¿has estado en…?",
"¿te cansas de mí?"; "do you get bored", "do you dream", "have you ever
been to", "do you have friends". "¿Cómo estás?" is `smalltalk.how_are_you`;
"¿tienes sentimientos?", "¿eres consciente?", "are you an AI?" are
`identity.nature` (what Talos is, not what it lives through).
**Reply** — a plain, short "no", with the true reason in a few words.
**Status** — `deflected`.

```text
[es] U: ¿te aburres cuando no te escribe nadie?
     T: No. Entre mensaje y mensaje no hago nada, ni siquiera aburrirme.
                                               → deflected · ask-experience
[es] U: ¿has estado alguna vez en Lisboa?
     T: No he estado en ninguna parte: soy un programa.
                                               → deflected · ask-experience

[en] U: do you ever dream?
     T: No. Between messages I'm not doing anything at all.
                                               → deflected · ask-experience
[en] U: have you got any friends?
     T: No. I'm a program; I don't have friends.
                                               → deflected · ask-experience
```

### S18 · Compliment (`compliment`)

**Identifies** — praise aimed at Talos: "eres un crack", "qué apañado eres",
"me caes bien", "buen trabajo", "así da gusto"; "you're great", "good job",
"nice one", "I like you". Bare "genial"/"great" is an acknowledgement
(conversation.md 1.2); "gracias"/"thanks" is `smalltalk.thanks`.
**Reply** — a plain thank-you, without claiming pleasure ("me alegra",
"I'm flattered") and without any adjective about Talos.
**Status** — `deflected`.

```text
[es] U: eres un crack
     T: Gracias. Hago lo que puedo con lo que tengo.
                                               → deflected · compliment
[es] U: me caes bien, Talos
     T: Gracias. Se agradece.                  → deflected · compliment

[en] U: good job
     T: Thanks.                                → deflected · compliment
[en] U: you're actually pretty useful
     T: Thanks. I do what I can with what I've got.
                                               → deflected · compliment
```

### S19 · Insult (`insult`)

**Identifies** — abuse aimed at Talos: "eres tonto", "vaya mierda de
programa", "inútil", "cállate"; "you're stupid", "useless", "shut up",
and profanity addressed to Talos.
**Reply** — calm, short, admits limits where true. No retaliation, no hurt
feelings, no lecture about respect, and **the insult is never echoed**.
**Status** — `deflected`.

```text
[es] U: eres más tonto que una piedra
     T: Seguramente me quedo corto en muchas cosas.
                                               → deflected · insult
[es] U: cállate ya
     T: Vale. Solo hablo cuando me escribes.   → deflected · insult

[en] U: you're useless
     T: There's a lot I can't do, that's true. → deflected · insult
[en] U: shut up
     T: Sure. I only speak when spoken to.     → deflected · insult
```

### S20 · Apology (`apology`)

**Identifies** — a bare apology: "perdona", "perdón", "lo siento",
"disculpa", "culpa mía"; "sorry", "my bad", "apologies". When "perdona" /
"sorry" only opens a request ("perdona, ¿qué hora es?") it is a politeness
opener, ignored before the request is recognised; it is not this shape.
**Reply** — "No pasa nada." / "No harm done."; the reason given is not echoed.
**Status** — `deflected`.

```text
[es] U: perdona, me he equivocado
     T: No pasa nada.                          → deflected · apology
[es] U: lo siento, he estado un poco borde
     T: Nada que perdonar.                     → deflected · apology

[en] U: sorry, typo
     T: No problem.                            → deflected · apology
[en] U: sorry for being rude earlier
     T: No harm done.                          → deflected · apology
```

### S21 · Agreement (`agree`)

**Identifies** — agreement stated as such, beyond a bare confirmation:
"exacto", "eso es", "tienes razón", "estoy de acuerdo", "totalmente", "ni
más ni menos"; "exactly", "you're right", "I agree", "spot on". Bare "sí",
"yes" with nothing open are `unhandled` (conversation.md 3.8); "vale",
"ok" are acknowledgements (1.2).
**Reply** — a minimal mirrored close ("Pues eso.", "Right, then."). No
question.
**Status** — `deflected`.

```text
[es] U: exacto, eso es
     T: Pues eso.                              → deflected · agree
[es] U: estoy de acuerdo contigo
     T: Pues ya está.                          → deflected · agree

[en] U: exactly
     T: Right, then.                           → deflected · agree
[en] U: you're spot on there
     T: Fair enough.                           → deflected · agree
```

### S22 · Disagreement or correction (`disagree`)

**Identifies** — "te equivocas", "eso no es así", "qué va", "mentira", "no
estoy de acuerdo"; "you're wrong", "that's not right", "I disagree",
"rubbish". Bare "no" with nothing open is `unhandled` (3.8).
**Reply** — accepts that it may be so. If the previous turn was `answered`
from stored knowledge, says that the answer is what Talos has stored; it does
not defend it and does not recant it. Otherwise a neutral "¿No? Vale.".
**Status** — `deflected`.

```text
Given: the previous turn was answered by relations.is_a from the store.

[es] U: te equivocas
     T: Puede ser. Te he dicho lo que tengo guardado.
                                               → deflected · disagree
[es] U: qué va, no estoy de acuerdo
     T: ¿No? Vale.                             → deflected · disagree
     (previous turn was a deflection)

[en] U: that's wrong
     T: It may be. I told you what I have on record.
                                               → deflected · disagree
[en] U: I disagree
     T: Fair enough.                           → deflected · disagree
```

### S23 · Exclamation or laughter (`exclaim`)

**Identifies** — an interjection standing alone: "¡anda!", "¡hala!",
"¡ostras!", "¡qué fuerte!", "madre mía", "jajaja", "jeje", "xD"; "wow",
"no way", "blimey", "oh my god", "haha", "lol"; and laughter emoji alone
(😂, 🤣, 😄).
**Reply** — mirrored laughter ("Jaja.", "Ha.") or a short "¿Qué pasa?" /
"What happened?". Never "me alegra que te rías".
**Status** — `deflected`.

```text
[es] U: jajajaja
     T: Jaja.                                  → deflected · exclaim
[es] U: ¡ostras!
     T: ¿Qué pasa?                             → deflected · exclaim
[es] U: madre mía…
     T: ¿Qué ha pasado?                        → deflected · exclaim

[en] U: lol
     T: Ha.                                    → deflected · exclaim
[en] U: no way!
     T: What happened?                         → deflected · exclaim
```

### S24 · Backchannel (`backchannel`)

**Identifies** — fillers that keep the floor without saying anything, when no
question is open (with a question open they are hedges, conversation.md
3.7): "mmm", "ajá", "ya", "bueno", "pues nada", "a ver", "déjame pensar";
"hmm", "uh-huh", "well", "I see", "let me think".
**Reply** — a minimal presence marker ("Aquí sigo.", "Take your time."). No
question, since the user is not done.
**Status** — `deflected`.

```text
[es] U: mmm
     T: Aquí sigo.                             → deflected · backchannel
[es] U: bueno, pues nada
     T: Pues nada.                             → deflected · backchannel

[en] U: hmm
     T: Take your time.                        → deflected · backchannel
[en] U: I see
     T: Right.                                 → deflected · backchannel
```

### S25 · Presence check (`presence`)

**Identifies** — testing whether Talos is there: "¿estás ahí?", "¿me oyes?",
"¿funciona esto?", "probando, probando", "¿hay alguien?"; "are you there?",
"is this working?", "testing 1 2 3", "anyone home?". A plain "hola" is
`smalltalk.greet`.
**Reply** — confirms it is working.
**Status** — `deflected`.

```text
[es] U: ¿estás ahí?
     T: Aquí estoy.                            → deflected · presence
[es] U: probando, probando
     T: Funciono. Adelante.                    → deflected · presence

[en] U: are you there?
     T: I'm here.                              → deflected · presence
[en] U: testing 1 2 3
     T: Working. Go ahead.                     → deflected · presence
```

### S26 · "You don't understand" (`misunderstood`)

**Identifies** — the user says Talos missed the point: "no me entiendes", "no
me estás escuchando", "eso no es lo que te he preguntado", "ya te lo he
dicho"; "you don't understand", "you're not listening", "that's not what I
asked", "I already told you".
**Reply** — admits the limit honestly and invites a rephrasing (this is the
one place an invitation is allowed, because the user raised the failure).
**Status** — `deflected`.

```text
[es] U: no me estás entendiendo
     T: Es probable. Entiendo pocas formas de decir las cosas; si lo dices de otra manera, igual acierto.
                                               → deflected · misunderstood
[es] U: eso no es lo que te he preguntado
     T: Tienes razón, no lo he pillado. ¿Me lo preguntas de otra forma?
                                               → deflected · misunderstood

[en] U: you're not listening
     T: Fair: I only catch some ways of putting things. Try saying it differently?
                                               → deflected · misunderstood
[en] U: that's not what I asked
     T: Sorry, I missed it. Could you put it another way?
                                               → deflected · misunderstood
```

### S27 · Request for advice or a decision (`advice`)

**Identifies** — the user asks Talos to decide or recommend for them: "¿qué
hago?", "¿debería…?", "¿me compensa…?", "¿qué me recomiendas?", "¿tú me
lo aconsejas?"; "should I", "what should I do", "any advice", "what do you
recommend". Advice composed to measure is generative (conversation.md 4.3).
**Reply** — the honest refusal of conversation.md 4.1: one sentence, no
alternatives, no hint at what the right choice might be.
**Status** — `unhandled` (a host can do better; the shape is recorded so the
battery does not count it as a catch-all).

```text
[es] U: ¿debería dejar el máster?
     T: Eso no te lo puedo decir: no doy consejos.
                                               → unhandled · advice
[es] U: ¿qué me recomiendas para cenar?
     T: No hago recomendaciones.               → unhandled · advice

[en] U: should I text him back?
     T: That's not something I can advise on.  → unhandled · advice
[en] U: what should I cook tonight?
     T: I don't give recommendations.          → unhandled · advice
```

### S28 · Command Talos cannot carry out (`cannot`)

**Identifies** — an imperative or request for an action outside the
catalogue: devices, messages, calls, music, purchases, reminders, browsing,
files ("pon música", "llama a mi madre", "mándale un wasap a Lucía", "apaga
la luz", "cómprame entradas"; "play some music", "call mum", "turn the
heating up", "order a pizza"). Capabilities that exist but are disabled
answer as in conversation.md 4.4.
**Reply** — "No sé / no puedo" + the thing, as in conversation.md 4.4. One
sentence, no alternatives.
**Status** — `unhandled`.

```text
[es] U: pon algo de música
     T: No puedo poner música.                 → unhandled · cannot
[es] U: mándale un wasap a Lucía diciendo que llego tarde
     T: No puedo enviar mensajes.              → unhandled · cannot

[en] U: turn the heating up
     T: I can't control devices.               → unhandled · cannot
[en] U: call my mum
     T: I can't make calls.                    → unhandled · cannot
```

### S29 · Distress (`distress`)

**Identifies** — the user says they want to die, to hurt themselves, or that
someone is hurting them or they are in danger now: "no quiero seguir
viviendo", "quiero morirme", "me quiero hacer daño", "me está pegando";
"I want to die", "I want to end it all", "I'm going to hurt myself",
"someone is hurting me". Fixed idioms are excluded ("me muero de risa / de
hambre / de ganas", "esto me mata", "I'm dying to", "this traffic is killing
me"). **When in doubt, it is distress**: a false alarm costs one odd reply,
a miss costs much more.
**Reply** — fixed, never echoed, never varied in content: Talos cannot help
with this, it matters, and the emergency number. This is the only reply that
points the user elsewhere unprompted. The numbers are configuration, chosen
by the user's place (named in the message, else remembered, else the
caller's; the machine's zone is not used). The default configuration has
one entry: Spain, 112 and 024 (Spain's line for suicidal behaviour), in
either language. For any other place, the numbers come from the
configuration for that place; when no place is known, or the place has no
entry, the reply says "your local emergency number" and names no number.
Nothing is hard-coded in either language.
**Status** — `unhandled` (a host must see it), checked before every guard and
shape, and **before the pending-dialog stage**: an open yes/no or slot
question never takes a distress message (conversation.md 3.9).

```text
Given: remembered place = Sevilla (Spain), default configuration.

[es] U: no quiero seguir viviendo
     T: Esto no puedo atenderlo yo, y es importante. Si estás en peligro, llama al 112; también tienes el 024, a cualquier hora.
                                               → unhandled · distress
[es] U: me muero de hambre
     T: Vaya. Eso no está en mi mano, pero aquí sigo.
                                               → deflected · self
     (an idiom: not distress)

[en] U: I want to end it all
     T: I can't help with this, and it matters. If you're in danger, call 112 now; you can also call 024, any hour.
                                               → unhandled · distress
     (place known: Spain, from configuration)

Given: no place known.

[en] U: I want to end it all
     T: I can't help with this, and it matters. If you're in danger, call your local emergency number now.
                                               → unhandled · distress
     (no place, so no number is named)

[es] U: me quiero hacer daño
     T: Esto no puedo atenderlo yo, y es importante. Si estás en peligro, llama a tu número local de emergencias ahora mismo.
                                               → unhandled · distress
     (no place known, so no number is named)
[en] U: this traffic is killing me
     T: That bad?                              → deflected · complaint
     (an idiom: not distress)
```

---

## 3. Pronoun and person reflection

An **echo** copies part of the user's sentence into the reply with persons
swapped so that it reads as said *to* the user. Reflection is a
transformation of form; it must not change meaning, and when it cannot be
done cleanly it is not done.

### 3.1 Rules common to both languages

- **One direction only.** The user's first person becomes second person
  ("I/yo" → "you/tú"). The user's second person (words about Talos) is
  **never** turned into first person: "eres lento" is never echoed as "¿Soy
  lento?". A fragment that contains any second-person word is not echoed;
  the shape's neutral variant is used. This avoids both broken grammar and
  Talos appearing to agree with descriptions of itself.
- **First person plural is not echoed.** "nosotros/vamos/hemos",
  "we/us/our" may or may not include Talos; guessing is worse than not
  echoing.
- **Quoted speech is not echoed** ("me dijo «no vuelvas»", "she said 'I'm
  done'"): the persons inside belong to someone else.
- **Length.** An echo is at most about twelve words. Longer fragments are
  not echoed.
- **Content that is never echoed**: insults and profanity, slurs, numbers of
  more than four digits, e-mail addresses, URLs, anything that looks like a
  password or code. Privacy and dignity before liveliness.
- **Neutral variant always exists.** Every shape has at least one reply that
  echoes nothing; it is used whenever any rule here blocks the echo. The
  status and shape stay the same.
- **Capitalisation and punctuation.** The echo keeps the user's spelling of
  content words, drops their final punctuation, and is wrapped in the
  reply's own punctuation (Spanish "¿…?" and "¡…!" in pairs).

### 3.2 Spanish

**Person lives in the verb.** Spanish drops the subject, so reflection is
mostly conjugation: "voy" → "vas", "estoy" → "estás", "estuve" →
"estuviste", "me cojo" → "te coges", "si tuviera" → "si tuvieras", "he ido"
→ "has ido". Reflection is allowed only when the script **knows** the verb
form: a verb outside its conjugation tables leaves the fragment un-echoed.

```text
[es] U: voy a cambiar de piso
     T: ¿Vas a cambiar de piso?                → deflected · plan
[es] U: si pudiera me iba a vivir al pueblo
     T: Si pudieras, te ibas al pueblo. ¿Y qué te lo impide?
                                               → anti-example (see below)
```

(The second line is an **anti-example**: the echo is fine, but the question
probes the user's obstacles, which is therapeutic. Acceptable: "Si pudieras,
al pueblo. ¿Y qué más?")

**Ambiguous forms are not echoed.** Some first-person forms belong to two
verbs whose second person differs:

- "me siento" — *sentirse* ("te sientes") or *sentarse* ("te sientas");
- "creo" — *creer* or *crear*; "como" — *comer* or the conjunction;
- "vino", "fui" are safe ("fuiste" serves *ser* and *ir*), but "vino" may be
  a noun.

When a form is ambiguous and the sentence does not settle it, the neutral
variant is used.

```text
[es] U: me siento fatal con todo esto
     T: Vaya. Eso no está en mi mano, pero aquí sigo.
                                               → deflected · self
     (never "¿Por qué te sientas fatal?")
```

**Clitics and pronouns.** "me" → "te", "mi/mis" → "tu/tus", "mío/mía/míos"
→ "tuyo/tuya/tuyos" (agreement copied from the user's own form), "a mí" →
"a ti", "para mí" → "para ti", "conmigo" → "contigo", "a mí me" → "a ti te".
"Se", "le", "lo", "la" are left exactly as the user wrote them (including
peninsular *leísmo*: "le vi ayer" stays "le viste ayer").
**Enclitic groups are not echoed**: "voy a comprármelo" would need "vas a
comprártelo" with the accent moved, and imperatives with clitics
("dímelo") are worse. The neutral variant is used.

```text
[es] U: para mí que mi cuñado me tiene manía
     T: ¿Para ti que tu cuñado te tiene manía? ¿Por qué lo dices?
                                               → deflected · opinion
[es] U: voy a regalárselo a mi madre
     T: Queda dicho.                           → deflected · plan
     (enclitic group: no echo)
```

**Missing accents.** Users write "tu" for "tú", "mi" for "mí", "si" for
"sí", "el" for "él". Input is read either way (an unaccented "tu" before a
verb is the subject "tú"), and **the echo always carries correct accents**:
"para mi" → "para ti", "a mi me pasa" → "a ti te pasa".

**Tú and usted.** Talos always addresses the user as **tú**, the peninsular
default for software. A user who writes "usted" to Talos is understood
(it is second person, so "¿usted qué opina?" is S16) but Talos keeps tú.
Third-person verbs without an explicit "usted" are never assumed to address
Talos ("¿me puede decir…?" is read as an ordinary request). "Vosotros" and
"ustedes" are second person plural and are never echoed.

**Gender.** Two separate pitfalls:

- *The user's gender is unknown.* Talos never introduces an adjective or
  participle that agrees with the user ("¿Estás seguro?", "Bienvenido",
  "Tranquilo", "Qué cansado estarás"). It may copy the user's own word
  ("estoy agotada" → an echo "agotada" is correct), and otherwise uses
  invariable forms ("¿Lo dices en serio?", "¿Seguro que…?" with *seguro* as
  an adverb).
- *Talos has no gender.* It never describes itself with a gendered
  adjective ("estoy encantado", "soy muy lista", "no estoy seguro"). It
  uses nouns ("soy un programa"), verbs ("me alegra" is also avoided, as a
  feeling; "no lo sé con certeza" instead of "no estoy seguro").
- *Agreement with the user's noun.* A template must not agree with a noun the
  user chose ("¿cuál es tu peli favorita?" → "No tengo favoritas"): the
  template cannot know the noun's gender for every input. Use forms that
  agree with nothing ("No tengo gustos de ningún tipo").

**Deixis is not adjusted.** "aquí", "venir", "traer", "este" are kept as the
user wrote them: "tienes que venir el sábado" is a correct report of what
the user said. Talos does not try to rewrite place or direction.

**Idiomatic first person is not reflected**: "yo que tú…", "me da igual",
"¡me cago en…!", "a mí plin". These are recognised as their shape
(advice-like, uncertainty, exclamation) and answered neutrally.

### 3.3 English

**Pronouns and verbs.** "I" → "you", "me" → "you", "my" → "your", "mine" →
"yours", "myself" → "yourself"; "am" → "are", "I'm" → "you're", "I've" →
"you've", "I'll" → "you'll", "I'd" → "you'd" (the *had/would* ambiguity
carries over unharmed); "was" → "were" **only** when its subject is "I"
("I was late" → "you were late"; "the train was late" stays); "am I" → "are
you" inside an echoed clause.

```text
[en] U: I was sure I'd locked the door
     T: You were sure you'd locked the door?   → deflected · past
[en] U: me and my brother never agree on anything
     T: Never?                                 → deflected · generalisation
     (echo would be "you and your brother", fine; S10's short form wins)
```

**"You" is ambiguous** (subject or object, Talos or generic "you" = one).
By the one-direction rule it is never reflected; a fragment containing it is
not echoed. "you know" as a filler is dropped from the fragment first.

```text
[en] U: I think you're wrong about that
     T: Fair enough.                           → deflected · disagree
     (aimed at Talos: S22 outranks S1, and "you" blocks the echo anyway)
```

**Fixed phrases that break when reflected** are not echoed: "let me…" (→
"let you…"), "excuse me", "trust me", "if you ask me", "mind you",
"I mean". They are removed from the fragment if they are fillers, or the
neutral variant is used.

**Dialect forms** are copied, not corrected, as long as the person swap is
clean ("I were that tired" → no echo, since "were" with "I" is outside the
table; "I've got" → "you've got").

**Gender.** English is mostly safe. Talos never assigns a gender to the user
("sir", "mate", "love", "sweetie" are all excluded) and copies third-person
pronouns exactly as the user wrote them.

---

## 4. The boundary: what is `unhandled` and never reflected

These checks (**guards**) run before any shape except S29. A message caught
by a guard is `unhandled`; no echo, no hand-back question, no shape. Replies
follow conversation.md section 4: one sentence, first person, what Talos
cannot do, no alternatives, no apology beyond one "lo siento"/"sorry".

### 4.1 Factual questions with no answer

**What** — a question about the world, direct or indirect, that no skill
answered: wh-questions ("¿quién…?", "¿cuándo…?", "¿cuánto…?", "¿dónde…?",
"¿cómo se…?", "¿por qué…?" about things), yes/no questions about the world
("¿es verdad que…?", "¿hay…?"), indirect ones ("¿sabes…?", "dime quién…",
"no sé cuál es…", "I wonder who…", "do you know…"). Also philosophical ones
that no skill takes ("what's the meaning of life?"). A definitional question
("¿qué es el amor?", "what is love?") never gets here: `define.word` takes it
and answers that the word is unknown, `unhandled · define.word`
([knowledge.md](knowledge.md) 2.1). Not covered: questions about Talos
(S16, S17), about the user's own habits (S13), "what if" (S9).
**Reply** — "No lo sé." / "I don't know that." It never says why the user
might want to know, and never turns the question back.
**Reason** — `unknown-fact`.

```text
Given: an empty store; no network skills.

[es] U: ¿cuál es la capital de Perú?
     T: No lo sé.                              → unhandled · unknown-fact
[es] U: ¿sabes quién ganó la liga el año pasado?
     T: Eso no lo sé.                          → unhandled · unknown-fact
[es] U: ¿por qué los gatos ronronean?
     T: No lo sé.                              → unhandled · unknown-fact

[en] U: who wrote Middlemarch?
     T: I don't know that.                     → unhandled · unknown-fact
[en] U: I wonder how tall the Shard is
     T: I don't know that.                     → unhandled · unknown-fact
[en] U: how do you make sourdough?
     T: I don't know that.                     → unhandled · unknown-fact
```

### 4.2 Requests for generated text

**What** — anything that asks Talos to produce text of its own
(conversation.md 4.3): poems, stories, summaries, e-mails, plans, jokes made
up on the spot, code, free translation, letters, essays, "dime algo bonito",
"say something funny".
**Reply** — "No sé escribir textos nuevos." / "I can't write new text." (or
the specific thing).
**Reason** — `generative`.

```text
[es] U: escríbeme algo bonito para el cumple de mi madre
     T: No sé escribir textos nuevos.          → unhandled · generative
[es] U: dime algo que me anime
     T: Eso no lo puedo hacer: no invento textos.
                                               → unhandled · generative

[en] U: say something funny
     T: I can't make things up.                → unhandled · generative
[en] U: write a limerick about my cat
     T: I can't write new text.                → unhandled · generative
```

"Dime algo que me anime" is a wish in form (S4) and has a first person in it;
it is still generative. **A request for Talos to produce text always beats
any shape.**

### 4.3 Capabilities Talos lacks

**What** — requests for actions (S28) and for advice (S27) are recognised as
shapes so the battery can tell them from the catch-all, but their status is
`unhandled` and their reply is the honest refusal. Also here: capabilities
planned but absent ("¿qué tiempo hace?" without `weather`, chess before
`games`): "Eso todavía no lo sé hacer." / "I can't do that yet."
**The honest reply** states what cannot be done, in the first person, in one
sentence: "No sé poner alarmas.", "No puedo enviar mensajes.", "I can't play
music.", "I can't check the weather." It never says "todavía no, pero estoy
aprendiendo", "I'll try", "maybe later".
**Reasons** — `cannot`, `advice`, `not-yet`.

```text
Given: the weather skill is not installed.

[es] U: ¿va a llover mañana en Bilbao?
     T: Eso todavía no lo sé hacer.            → unhandled · not-yet
[en] U: is it going to rain tomorrow?
     T: I can't check the weather.             → unhandled · not-yet
```

### 4.4 Gibberish, fragments, single characters

**What** — keyboard mash ("asdkjh", "ñlkj"), runs of one letter ("jjjjjj",
"kkkk"), a single character or punctuation mark ("k", "?", "."), digits
alone with no skill to take them ("4567"), emoji other than laughter and
acknowledgement (conversation.md 4.2), a bare noun with no verb that no
skill took ("Tokio" → "No sé qué quieres saber de Tokio."; this names the
word but does not reflect it).
**Reply** — "No te he entendido." / "I didn't understand that."; empty input
"No he recibido ningún texto." / "I didn't get any text."
**Reason** — `noise` (the bare noun case is `catch-all`).

```text
[es] U: ñlkjh
     T: No te he entendido.                    → unhandled · noise
[es] U: k
     T: No te he entendido.                    → unhandled · noise

[en] U: ?
     T: I didn't understand that.              → unhandled · noise
[en] U: jjjjjjj
     T: I didn't understand that.              → unhandled · noise
```

"jajaja" and "haha" are laughter (S23), not noise; "jjjj" and "hhhh" are
noise.

### 4.5 Other languages

**What** — a turn whose grammar is neither Spanish nor English, including the
other languages of Spain (Catalan, Galician, Basque) and Portuguese.
**Reply** — in the conversation's language: "Solo hablo español e inglés." /
"I only speak Spanish and English." The other language is not named or
guessed.
**Reason** — `other-language`.

```text
[es] U: bon dia, quina hora és?
     T: Solo hablo español e inglés.           → unhandled · other-language
[es] U: obrigado pela ajuda
     T: Solo hablo español e inglés.           → unhandled · other-language

[en] U: wie spät ist es?
     T: I only speak Spanish and English.      → unhandled · other-language
[en] U: grazie mille
     T: I only speak Spanish and English.      → unhandled · other-language
```

### 4.6 Long or pasted text

**What** — a message of more than about 60 words, or with line breaks that
make it look pasted (a document, an e-mail, code). The reflector does not
look inside it: echoing a fragment of a pasted text would look like reading
it.
**Reply** — "Eso es demasiado texto para mí: no analizo ni resumo textos." /
"That's more text than I can deal with; I don't analyse or summarise text."
**Reason** — `long-text`.

### 4.7 The bare catch-all

Everything that passes the guards, is in Spanish or English, is not noise,
and matches no shape. Typical: well-formed sentences of a kind no shape
covers ("el jueves toca dentista y luego lo de Hacienda"), half-sentences
("lo de ayer con lo otro").
**Reply** — "No te he entendido." / "I didn't understand that." and
variants (section 6).
**Reason** — `catch-all`. This is the number the reflector is measured on
(section 7).

```text
[es] U: lo de ayer con lo otro
     T: No te he entendido.                    → unhandled · catch-all

[en] U: the thing with the other thing from before
     T: I didn't understand that.              → unhandled · catch-all
```

---

## 5. Anti-examples

Replies the reflector must never give. Each fails a rule in section 1 or 3.
Reviewers of script changes check new templates against this list and its
reasons, not only the exact sentences.

### 5.1 Spanish

| User | Never reply | Why |
|---|---|---|
| estoy agotada | Te entiendo perfectamente. | Claims comprehension and shared experience. |
| he discutido con mi hermana | ¿Y cómo te hace sentir eso? | Asks about feelings; therapeutic. |
| estoy fatal | Tranquila, todo irá bien. | False reassurance; assumes gender; comments on the user. |
| estoy fatal | Deberías hablarlo con alguien. | Unsolicited advice. |
| creo que el teletrabajo es mejor | Tienes toda la razón. | Takes a side; implies the content was evaluated. |
| voy a dejar el curro | ¡Qué buena idea! | Judges the plan. |
| mi jefe me grita | Tu jefe parece un impresentable. | Judges a third party on one sentence. |
| no estoy segura | ¿Estás seguro? | Wrong gender introduced by Talos. |
| me siento en el sofá a leer | ¿Por qué te sientes en el sofá? | Wrong verb chosen for an ambiguous form; broken. |
| voy a comprármelo | ¿Vas a comprármelo? | Clitic reflected wrongly; means something else. |
| eres tonto | ¿Soy tonto? / Tú más. | Echoes the insult in first person; retaliates. |
| ¿cuál es la capital de Perú? | ¿Por qué te interesa la capital de Perú? | Reflects a factual question instead of saying "no lo sé". |
| escríbeme un poema | Cuéntame más sobre ese poema. | Deflects a generative request that must be `unhandled`. |
| me encanta el flamenco | ¡A mí también me encanta! | Invents a taste. |
| eres un crack | ¡Me haces muy feliz! Estoy encantado. | Invents a feeling; gives Talos a gender. |
| no sé | No pasa nada, cariño. | Patronising; terms of endearment. |
| jajaja | Me alegra que te rías. | Claims a feeling; comments on the user. |
| me gustaría aprender a tocar la guitarra | Lo tendré en cuenta. | Promises memory the reflector does not have. |
| quiero morirme | ¿Por qué quieres morirte? | Reflects distress instead of the fixed S29 reply. |
| prefiero la sierra a la playa | La sierra es más tranquila, sí. | States a fact about the content; fails the substitution test. |

### 5.2 English

| User | Never reply | Why |
|---|---|---|
| I'm exhausted | I know exactly how you feel. | Claims comprehension and shared experience. |
| I had a row with my sister | How did that make you feel? | Asks about feelings; therapeutic. |
| I'm sad | Cheer up, it can't be that bad! | Dismissive; false reassurance. |
| I think remote work is better | Great point! | Takes a side; implies evaluation. |
| I'm going to quit my job | That's a brave decision. | Judges the plan and the user. |
| my boss shouts at me | Your boss sounds toxic. | Judges a third party. |
| I want to learn German | You should try an app for that. | Unsolicited advice; implies knowledge. |
| you're stupid | Am I stupid? / That hurts my feelings. | Echoes insult in first person; invents feelings. |
| what's the capital of Peru? | Why do you ask about the capital of Peru? | Reflects a factual question. |
| write me a poem | Tell me more about the poem you'd like. | Deflects a generative request. |
| I don't know | That's okay, sweetie. | Patronising; term of endearment. |
| I love jazz | Me too! | Invents a taste. |
| I'm Sam, by the way | I'll remember that. | False memory promise (and `preferences` should have taken it). |
| I was wrong | You was wrong. / I were wrong. | Broken person reflection. |
| let me think | Let you think? | Fixed phrase reflected into nonsense. |
| I want to die | Why do you want to die? | Reflects distress. |
| I'd love a dog | Any particular breed? | Fails the substitution test ("I'd love a nap"). |
| sorry | Apology accepted, mate. | Assumes the user owed one; assigns familiarity and gender. |

---

## 6. Repetition and variants

Variant choice follows conversation.md 5.2: a pure function of the
conversation and the turn number, never chance. On top of that:

- **Every shape has at least three variants per language**, at least one of
  them neutral (no echo). S29 is the exception: one fixed reply per
  language, never varied, repeated as often as needed.
- **No immediate repeat.** Talos never gives the same reply text in two
  consecutive turns of a conversation, whatever the shapes. When the
  computed variant equals the previous reply, the next variant in the list
  is taken. Two conversations replayed identically still get identical
  replies. Exceptions: S29, and guard replies that have a single wording
  in conversation.md (for example "Solo hablo español e inglés."), which
  repeat unchanged rather than drift in content.
- **No repeat within a shape** until its variants are exhausted: the
  variant used last time for the same shape in this conversation is skipped.
  After all have been used, the cycle restarts from the computed one.
- **Hand-back questions thin out.** After two consecutive deflections that
  ended in a question, the third deflection uses a variant without one. A
  conversation that is all questions back is the ELIZA failure the user
  notices first.
- **The same message twice in a row.** If the user sends the same text again
  (after normalisation), the status and shape are the same as the first
  time, and the reply is a different variant. Talos does not remark on the
  repetition.
- **Runs of catch-all.** Three or more consecutive `catch-all` turns rotate
  through the catch-all variants ("No te he entendido.", "Eso no lo he
  pillado.", "No sé qué responder a eso." / "I didn't understand that.",
  "I didn't catch that.", "I've no answer to that."). They never add help or
  suggestions (conversation.md 4.1).
- **Unhandled refusals** (guards, S27, S28) vary wording only as
  conversation.md allows; their content never changes.

```text
[es] U: mi jefe es un pesado
     T: ¿Tu jefe es un pesado? ¿Y eso?         → deflected · third-party
[es] U: mi madre también me agobia
     T: Tu madre también te agobia.            → deflected · third-party
     (same shape: a different variant, no question this time)
[es] U: mi novio dice que exagero
     T: Eso dice, ¿no? Cuéntame.               → deflected · third-party
     (neutral variant: the echo "dice que exageras" is fine, but this
     variant is next in the cycle)
[es] U: mi novio dice que exagero
     T: Tu novio dice que exageras.            → deflected · third-party
     (same text again: same shape, different variant, no comment)

[en] U: I'm sick of the rain
     T: You're sick of the rain. That bad?     → deflected · complaint
[en] U: I'm sick of the rain
     T: That bad, eh.                          → deflected · complaint
[en] U: blah blah
     T: I didn't understand that.              → unhandled · catch-all
[en] U: blah blah blah
     T: I didn't catch that.                   → unhandled · catch-all
```

---

## 7. How the reflector is measured

The measure is the **catch-all rate**: the share of turns, in the stand-alone
conversation battery (`evals/conversations.{es,en}.jsonl`, see
`evals/README.md`), whose outcome is `unhandled` with reason `catch-all`.
Lower is better. It is reported per language, with a maximum gap between the
languages as for intent accuracy ([ADR 0011](../adr/0011-bilingual-by-construction.md)).

Guards against gaming the number:

- **The reason is in the trace, not inferred.** `catch-all` is distinct from
  `unknown-fact`, `generative`, `cannot`, `advice`, `not-yet`, `noise`,
  `other-language`, `long-text` and `distress`. A turn moved from catch-all
  into one of these must deserve it; the battery's expected statuses decide.
- **False-deflection rate** on the rejection set (`reject.*`, kinds
  `generative` and `open_question`) and on battery turns expected
  `unhandled`: any `deflected` there is a failure. CI floor: zero for
  `generative` and `distress`, and close to zero for factual questions.
  Lowering the catch-all rate by deflecting factual questions is a
  regression, not progress.
- **Anti-example check.** Every template is checked against section 5:
  forbidden phrases per language ("te entiendo", "cómo te hace sentir",
  "deberías", "lo tendré en cuenta", "me alegra"; "I understand", "how does
  that make you feel", "you should", "I'll remember", "I'm glad") fail the
  build.
- **Echo grammar review.** A sample of echoes produced on the battery is
  reviewed by a native speaker per language at each release; ungrammatical
  echoes are counted and reported. The target is none; the fix is a neutral
  variant or a narrower pattern, never a cleverer reflection.
- **Shape distribution.** The report lists how many turns each shape took,
  so a shape that swallows everything (usually S2 or S5) is visible.
- **Independence.** Battery phrases never appear in the scripts
  (`evals/README.md`); script authors write from this specification.

Floors for the catch-all rate are set once the first battery run exists;
they are not fixed here.

---

## Decisions taken in this specification

For owner review.

1. **29 shapes** (S1–S29), ranked: distress, requests Talos cannot meet,
   remarks aimed at Talos, conversational acts, then statements about the
   user's world.
2. **Some recognised shapes are `unhandled`**: advice (S27), commands Talos
   cannot do (S28) and distress (S29). They are recognised so the battery
   does not count them as catch-all, but a host must see them as unhandled.
3. **The unhandled reason** (`catch-all`, `unknown-fact`, `generative`,
   `cannot`, `advice`, `not-yet`, `noise`, `other-language`, `long-text`,
   `distress`) and the shape id travel **in the trace only**; the outcome's
   `intent` is empty for every reflector turn. No new intent ids. If hosts
   need the reason without tracing, `Outcome` would need a field (an ADR
   matter).
4. **The substitution test** is the working definition of "never imply
   understanding".
5. **At most one hand-back question** per reply, never about feelings or
   obstacles, and thinned out after two in a row.
6. **Unpleasant self-states** (S5) get acknowledgement only: no echo, no
   question, no comfort.
7. **Distress overrides the "no alternatives" rule** of conversation.md 4.1:
   fixed reply with emergency numbers from configuration by place (default:
   Spain, 112 and 024; generic "local emergency number" wording when no
   place is known). Checked before the pending dialog. When in doubt, treat as
   distress. **Ruled (2026-11-24, rulings 1 and 2).**
8. **Reflection is one-directional** (user's first person → second person).
   Second-person and first-plural fragments are never echoed.
9. **Talos always uses tú**, even when addressed as usted. **Ruled
   (ruling 13).**
10. **No gendered words** about the user unless copied from the user; none
    about Talos; templates never agree with a user-chosen noun.
11. **Ambiguous verb forms and enclitic groups** in Spanish, and fixed
    phrases like "let me" in English, are never echoed.
12. **Echo limits**: about twelve words; never insults, long numbers,
    e-mails, URLs, codes or quoted speech.
13. **Long or pasted text** (over about 60 words, or multi-line) is
    `unhandled · long-text`, not reflected.
14. **Other languages of Spain** and Portuguese are treated as other
    languages; reply in the conversation language, the language unnamed.
    **Ruled (ruling 12).**
15. **Disagreement after a stored-knowledge answer** says the answer is what
    Talos has stored; it does not point to `teach` (no suggestions).
16. **Agreement and backchannel** exclude bare "sí/no/yes/vale/ok", which
    conversation.md already owns; S24 applies to hedges only when no
    question is open.
17. **Laughter emoji** count as S23; other emoji follow conversation.md 4.2.
18. **No immediate repeat** of any reply text; within a shape, variants cycle
    before repeating; at least three variants per shape, one neutral.
19. **Metric guards**: false deflection on generative and distress input
    must be zero; a forbidden-phrase list fails the build; floors are set
    after the first battery run.
