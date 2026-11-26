# Knowledge

> **Built in two steps** (owner's ruling, 2026-11-25). v0.3 builds the store,
> the packs and what a dictionary needs: words, concepts, glosses,
> translation. Reasoning over relations, teaching facts, confidence and
> contradiction are deferred until after 1.0, on the same schema. See [ROADMAP.md](ROADMAP.md).

The knowledge store is what makes Talos semantic instead of a command bot. It
holds what words mean, how concepts relate, where each fact came from and how
much it is trusted — and it can be queried, reasoned over, explained and
taught.

Decisions: [ADR 0005](adr/0005-sqlite-knowledge-store.md),
[ADR 0006](adr/0006-knowledge-packs.md),
[ADR 0011](adr/0011-bilingual-by-construction.md).

## 1. Model

```
 lexeme (es: "guitarra") ──sense──┐
                                  ├──▶ concept ──claim(is_a)──▶ concept ◀──sense── lexeme (en: "instrument")
 lexeme (en: "guitar")  ──sense──┘
```

- A **concept** is a language-neutral node. It has a stable text identifier.
- A **lexeme** is a word in one language. Its inflected **forms** point back to
  it, so "guitarras" resolves to "guitarra".
- A **sense** links a lexeme to a concept and carries a gloss in that
  language. A word with several meanings has several senses.
- A **claim** is a relation between a subject concept and either another
  concept or a literal, with a predicate, a confidence, a source and an
  optional validity interval.
- A **source** says where something came from: the user, a pack, an import.

Consequences of this shape:

- Translation of single words is a lookup: two lexemes that share a concept.
- A fact taught in Spanish is known in English, because the claim is between
  concepts.
- Ambiguity is explicit: "banco" has senses, not a single definition.

## 2. Schema

```sql
CREATE TABLE source (
  id        INTEGER PRIMARY KEY,
  kind      TEXT NOT NULL,          -- 'user' | 'pack' | 'import'
  name      TEXT NOT NULL,
  license   TEXT,
  url       TEXT,
  added_at  INTEGER NOT NULL
);

CREATE TABLE concept (
  id        TEXT PRIMARY KEY,       -- stable: 'wn:03467517-n', 'user:01J…'
  kind      TEXT                    -- 'entity' | 'class' | 'property' | …
);

CREATE TABLE lexeme (
  id        INTEGER PRIMARY KEY,
  lang      TEXT NOT NULL,          -- 'es' | 'en'
  lemma     TEXT NOT NULL,
  pos       TEXT,
  UNIQUE (lang, lemma, pos)
);

CREATE TABLE form (
  lexeme_id INTEGER NOT NULL REFERENCES lexeme(id),
  form      TEXT NOT NULL,          -- normalised surface form
  features  TEXT                    -- 'pl', 'f', …
);
CREATE INDEX form_lookup ON form(form);

CREATE TABLE sense (
  id         INTEGER PRIMARY KEY,
  lexeme_id  INTEGER NOT NULL REFERENCES lexeme(id),
  concept_id TEXT    NOT NULL REFERENCES concept(id),
  gloss      TEXT,
  rank       INTEGER NOT NULL DEFAULT 0,   -- baseline order of senses
  source_id  INTEGER NOT NULL REFERENCES source(id)
);

CREATE INDEX sense_by_lexeme  ON sense(lexeme_id);
CREATE INDEX sense_by_concept ON sense(concept_id);

CREATE TABLE predicate (
  id         INTEGER PRIMARY KEY,
  name       TEXT UNIQUE NOT NULL,   -- 'is_a', 'part_of', 'causes', …
  inverse    TEXT,                   -- 'has_part' for 'part_of'
  transitive INTEGER NOT NULL DEFAULT 0,
  symmetric  INTEGER NOT NULL DEFAULT 0,
  functional INTEGER NOT NULL DEFAULT 0    -- at most one object per subject
);

CREATE TABLE claim (
  id          INTEGER PRIMARY KEY,
  subject     TEXT    NOT NULL REFERENCES concept(id),
  predicate   INTEGER NOT NULL REFERENCES predicate(id),
  object      TEXT    REFERENCES concept(id),
  literal     TEXT,                  -- when the object is a value
  negated     INTEGER NOT NULL DEFAULT 0,
  confidence  REAL    NOT NULL,
  source_id   INTEGER NOT NULL REFERENCES source(id),
  valid_from  INTEGER,
  valid_to    INTEGER,
  created_at  INTEGER NOT NULL,
  retracted_at INTEGER
);
CREATE INDEX claim_forward ON claim(subject, predicate);
CREATE INDEX claim_reverse ON claim(object, predicate);

CREATE VIRTUAL TABLE gloss_fts USING fts5(lemma, gloss, content='');

CREATE TABLE audit (
  id INTEGER PRIMARY KEY, at INTEGER NOT NULL,
  action TEXT NOT NULL, claim_id INTEGER, detail TEXT
);

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);  -- schema version
```

The schema is versioned in `meta`. Migrations are forward-only and run at
start-up on `user.db`; packs declare the schema version they were built for.

### As built (schema version 1, v0.3)

The tables above are in `talos-core/src/knowledge/schema.rs`, as written,
relations and audit included although nothing reasons over them before 1.0, with
these settlements:

- **No `gloss_fts` yet.** Full-text search is not part of v0.3 and FTS5 is
  compiled out of the bundled SQLite (117 KB); the version that brings search
  adds the table by migration.
- **Three more tables in every database**, which the design names without
  spelling out: `phrase` (a stock phrase's wordings: `entry` groups both
  sides, `lang`, `text` as written, `form` in the match form, `rank`,
  `approximate`, `source_id`; indexed on `form` and `entry`), `phrase_note`
  (when a wording is the one to use, per language: «al despedirse» /
  "when leaving") and `word_pair` (the lexicon of §7: `lang`, `word`, `form`,
  `target`, `rank`, `source_id`; indexed on `form, lang`). A pack without
  phrases or pairs leaves them empty.
- **`user.db` alone** has `preference` (key, value) and `correction` /
  `correction_slot` (phrase, intent, slot values): what was `user.toml`.
- **`meta`** holds `schema_version` and `kind` (`user` or `pack`), and in a
  pack its manifest (§7). Every Talos database has `PRAGMA application_id`
  `0x544C4F53` ("TLOS").
- **`form.form` is the match form** (lower case, no accents, single spaces,
  no end punctuation), the form every lookup is made in, so "violin" finds
  *violín*. Each lexeme has one `form` row for its lemma (`features` null)
  and one per exception no rule gives (`features` such as `pl`, `pres,3sg`);
  regular forms are not stored ("Inflected forms" below).
- **`sense.rank`** is the baseline order of a word's senses (§5). The order
  of a concept's words in one language (which one names it) is rank, then
  the order the pack wrote them.
- Foreign keys are off: the `REFERENCES` clauses document, the pack builder
  and `talos packs check` verify.
- **One more index, `form_by_lexeme ON form(lexeme_id)`** (Slice C): a
  word's form with given features ("the plural of *perro*") is read through
  its lexeme; without it each such read scanned every form of the
  vocabulary pack (10 ms).

### Inflected forms

A typed word is read back to its lemma by rules in
`talos-core/src/knowledge/inflect.rs`, tables of endings run both ways:

- English: plural and third person (`-s`, `-es`, `-ies`, `-ves`), `-men`,
  `-ing`, `-ed`, `-ied` (with a doubled consonant undone and a dropped `e`
  restored), comparative and superlative. A phrase inflects its last word.
- Spanish: plural (`-s`, `-es`, `-ces` → `-z`), gender (`-a` → `-o`,
  `-ora` → `-or`, and their plurals), the regular endings of the three
  conjugations (present, preterite, imperfect, present subjunctive, future,
  conditional, gerund, participle). A phrase inflects every word before its
  first preposition ("guitarras eléctricas", "cajas de resonancia").

A rule's candidate counts only when a lexeme with that lemma and a part of
speech the rule applies to exists (a verb ending never reads a noun), so a
rule that over-generates costs a lookup, not a wrong answer. Forms no rule
gives are data: `form` rows of a pack ("fish" plural of *fish*, "llueve" of
*llover*). The vocabulary pack extends them by writing rows (the wordnets'
exception lists), never by changing the rules. The same rules write a
regular plural for translation ("violines", "guitarras eléctricas"); a
stored exception comes first, in the match form.

As built (Slice C): the vocabulary pack carries Open English WordNet's
exception lists, the `Form` elements of its entries, as 4 451 rows ("geese",
"went", "better", "lain"). The lists do not say which form a form is, so
the features are read from the part of speech and the ending: `pl` for a
noun; `ger`, `pres,3sg`, `pres` (am, are) or `past` (past or participle) for
a verb; `sup` or `cmp` for an adjective or adverb. **Spanish has no
exception rows**: the Spanish wordnet carries no forms, and no table of
irregular verbs has been written, so "fue", "tengo", "hizo" are not found.
Writing them (or taking them from an attribution-only source) is open.

## 3. Queries Talos answers

| Question | Mechanism |
|---|---|
| What is X? / ¿Qué es X? | senses of the lexeme, ranked |
| Is X a Y? | transitive closure over `is_a`, bounded |
| What are the parts of X? / What is X part of? | `part_of` forward and reverse |
| What kinds of X are there? | reverse `is_a`, one or more levels |
| What does X cause? Why X? | `causes` chain, bounded |
| How do you say X in English/Spanish? | lexemes sharing a concept |
| What do you know about X? | neighbourhood of the concept |
| How sure are you? Who told you? | confidence and source of the claims used |

## 4. Reasoning

Reasoning is deliberately modest and always bounded.

- **Transitivity** for predicates flagged `transitive`, as a recursive query
  with a depth limit (default 6) and cycle protection.
- **Inheritance**: properties of a class apply to its subclasses and instances
  unless a more specific claim says otherwise.
- **Inverse and symmetric** predicates are answered from one stored direction.
- **Confidence**: a derived answer's confidence is the product of the claims
  on its path, with a small decay per hop. Independent sources asserting the
  same claim combine by noisy-OR.
- **Negative answers** distinguish "no, and here is the contradicting claim"
  from "I have nothing that says so".
- **Contradictions** are detected at teaching time: a second object for a
  functional predicate, a claim and its negation, or a cycle in `is_a`. Talos
  asks instead of overwriting; a cycle in `is_a` is refused outright, since
  no answer to a question could make it valid.
- **Time**: claims outside their validity interval at `turn.now` are ignored
  for present-tense questions.

Every derived answer carries its **path** — the claims that produced it — so
the reply can explain itself: *"Yes: a guitar is a string instrument, and a
string instrument is a musical instrument."*

## 5. Polysemy

When a word has several senses:

1. Senses are ranked by baseline `rank`.
2. If the turn or the recent conversation supplies context words, senses are
   re-ranked by overlap with their gloss and their neighbouring concepts.
3. If one sense wins clearly, Talos answers with it and mentions that others
   exist.
4. If not, it asks which one is meant (an *expectation*), and remembers the
   choice for the rest of the conversation.

Translation is the exception: it never asks, and gives one translation per
sense.

## 6. Teaching

```
teach: a theremin is an electronic instrument
aprende: el teremín es un instrumento electrónico
forget that a theremin is a string instrument
```

- Taught claims go to `user.db` with the user as source and a high but not
  absolute confidence.
- Unknown words create a lexeme, a sense and a `user:` concept.
- A user claim that contradicts a pack claim does not edit the pack: it is
  stored in `user.db` and overrides it at query time. Retracting a pack claim
  stores a tombstone.
- Everything taught is listed, exportable and reversible; the `audit` table
  records each change.

Automatic extraction of facts from free text is **not** part of the first
major version. Talos learns what it is told in a teaching form, not what it
overhears.

## 7. Packs

A pack is a read-only SQLite file with this schema and a manifest in `meta`
(`id`, `version`, `title`, `languages`, `license`, `notice`: the notices its
licence and its sources' licences ask for), its sources in `source`, and
`kind = pack`. It is built for exactly one schema version; a pack of another
version is refused. (The FTS index waits for search; see §2.)

Packs live in the data directory's `packs/` as `*.talospack` and are
attached at start in file-name order. Before attaching, each passes a
structural check: Talos's application id, the schema version, every table,
a complete manifest, an id no attached pack has. A pack that fails is left
out with a reason, shown on stderr, in `talos packs list` and in `/health`,
and Talos runs without it. `talos packs check FILE` adds SQLite's
page-by-page integrity check and the references the builder guarantees;
`talos packs info FILE` shows the manifest and the counts.

- Packs are attached at start-up, read-only and immutable, each with a small
  page cache of its own (256 KiB). No views are built: the dictionary
  queries run once per attached database, `user.db` first, and their results
  are combined in Rust; a word found in several databases is one word
  (lexemes match by language, lemma and part of speech), and a concept's
  gloss comes from the first database that has one.
- Recursive and joining queries are generated per attached database and their
  results combined; they are never written against the union views. The spike
  in [pack-attachment.md](research/pack-attachment.md) measured the same
  is-a question at tens of microseconds per database and at 80–95 ms through
  a view.
- Cross-pack references cannot be enforced by SQLite, so foreign keys are a
  build-time check of the pack builder, not a run-time constraint.
- The bundled SQLite allows ten attached databases, which bounds the number
  of packs in use at once (ten: `user.db` is the connection's own database).
- Concept identifiers are global text keys, so packs compose without
  renumbering.
- Packs are distributed separately from the binary. Their licence is their
  own: a pack built from a share-alike source carries that obligation without
  touching the code's licence.

### Pack sources

A hand-written pack is a TOML source built by `tools/pack`
(`cargo run -p talos-pack -- build SOURCE OUT`; with no arguments it builds
the seed, `knowledge/seed.toml`, into `target/packs/core.talospack`). The
build runs in memory and writes the file with `VACUUM INTO`, so the same
source gives the same bytes. Tests build the same source into an in-memory
database (`Knowledge::from_source`), so they answer from exactly the pack
that ships.

```toml
[pack]                    # the manifest; `source` names the source of what follows
id = "core"
version = "1.0"
title = "…"
languages = ["es", "en"]
license = "MIT"
notice = "…"
date = "2026-11-25"       # source.added_at, claim.created_at
source = "core"

[[source]]                # one per source: name, kind, license, url
name = "core"
kind = "pack"

[[predicate]]
name = "part_of"
inverse = "has_part"
transitive = true

[[concept]]
id = "core:rain"
kind = "class"            # default; "entity" for a named thing
es = ["lluvia", { lemma = "llover", pos = "v", forms = { llueve = "pres,3sg" } }]
en = ["rain", { lemma = "rain", pos = "v" }]
gloss.es = "agua que cae de las nubes en gotas"
gloss.en = "water falling from clouds in drops"

[relations]
claims = [
  { id = 35, s = "core:cloud", p = "causes", o = "core:rain", confidence = 0.8 },
  { id = 48, s = "core:pluto", p = "is_a", o = "core:planet", valid_to = "2006-08-24" },
]

[[phrase]]                # a stock phrase: both sides, wordings best first
id = "buenas_noches"
es = ["buenas noches"]
en = [{ text = "good night", note.es = "al despedirse", note.en = "when leaving" }]

[lexicon]                 # word pairs, under their own source
source = "lexicon"
pairs = [{ from = "es", word = "gaita", to = ["bagpipes", "bagpipe"] }]
```

A word is a lemma, or an inline table with `rank` (default 0), `pos` (`n`,
`v`, `adj`, `adv`; default `n`) and `forms` (the exceptions). A gloss is
copied onto every sense of a word of its language. The build refuses what
does not hold together: an unknown concept, predicate, source or language, a
gloss with no word in its language, a concept with no word, a duplicate.

The seed pack is the fixture of [docs/spec/knowledge.md](spec/knowledge.md)
§1, all of it: 63 concepts with their words in both languages (69 Spanish
and 75 English lexemes, 152 senses, every gloss the fixture does not leave
out), the 55 claims, the three predicates, the ten stock phrases and the
eight entries of the word-pair lexicon. The glosses the specification quotes
are copied from it; the others were written for the pack. It is one file
with two sources (`core` and `lexicon`), where the fixture installs two
packs; the lexicon of §7 proper is its own pack. The seed is shipped beside
the binary, not in it: it stands in for the core-vocabulary pack and would
collide with it (DEVELOPMENT.md, v0.3 Slice A).

### Sources for the first packs

| Pack | Source | Gives | When |
|---|---|---|---|
| Seed | hand-written | the everyday concepts the eval battery needs | v0.3, first |
| Core vocabulary | WordNet family (English WordNet + an aligned Spanish wordnet via Open Multilingual WordNet) | common words, glosses, is-a / part-of / antonym relations, es↔en alignment | v0.3, default |
| Entities | Wikidata subset | places, people, works; encyclopedic facts | after 1.0, optional |
| Lexicon | word-pair dictionary es↔en (see below) | the bridge between languages: translation, induced Spanish senses, detection | v0.3, default |

The WordNet family is the backbone because a synset *is* a concept in this
model: one meaning, the words that express it in each language, and typed
relations to other meanings. Wikidata plays the same role for entities and
slots into the same schema when it is wanted.

### The vocabulary pack, as built (v0.3 Slice C)

`talos-pack wordnet DIR` builds `vocabulary.talospack` from Open English
WordNet 2025+ and the MCR Spanish wordnet (as packaged by the Open
Multilingual Wordnet), joined through the CILI map of WordNet 3.0 offsets,
and, for Spanish glosses, Wikidata's Spanish descriptions (a CC0 snapshot in
`knowledge/sources/`) joined through the CILI map of WordNet 3.1.
How to get the files, their checksums, every count, what is dropped and
why, and the measurements are in [DEVELOPMENT.md](DEVELOPMENT.md), "The
vocabulary pack". In short:

- **120 564 concepts**, one per English synset, keyed by their CILI id
  (`i46360`; the 3 213 synsets without one keep their own id); 161 814
  English and 93 694 Spanish lexemes; 357 882 senses; 155 315 claims
  (`is_a`, `instance_of`, `part_of`, `member_of`, `substance_of`, `causes`,
  `similar_to`, `antonym_of`), which nothing reasons over until after 1.0.
- **The join** keeps 78 215 of the 78 417 Spanish synsets (99.7 %): 202
  name a CILI id that Open English WordNet no longer carries.
- **Glosses by source.** Every English sense has its synset's definition
  (source `oewn`); 18 853 concepts have a Spanish one from the MCR (source
  `mcr-spa`), 24 % of the 78 215 with Spanish words, and 18 516 more have
  Wikidata's Spanish description (source `wikidata`, never over an MCR
  gloss): 37 369 in all, 48 %. The Wikidata ones are short labels by
  contributors, nouns almost only, and a description may
  sit on another sense than the one ranked first (wordnet-sources.md
  §2.3.2). A sense's source is its gloss's source, so `Gloss::source` says
  which one a definition came from.
- **Sense order.** English: WordNet's order within an entry, commonest
  first. Spanish: none in the source; borrowed from the English word that
  most often translates the Spanish one, a guess documented with the
  builder. Lexemes that share a match form (a noun and a verb, "CAT" and
  "cat") are ordered between themselves so that one of them wins at rank 0.
- **Examples, pronunciations and the derivational relations are not
  stored**: schema version 1 has no place for them.
- **No word pairs and no induced senses.** The word-pair lexicon below has
  no settled source yet; when it has, its pairs go in a pack of their own
  and the senses it induces for Spanish words into this pack under a third
  source.

The notice it carries, in its manifest and in `vocabulary.NOTICE` beside it,
is the one of [wordnet-sources.md](research/wordnet-sources.md) §4 for the
2025+ edition, with what was changed and which licence covers which part,
and Wikidata (CC0, which adds no obligation): no single outbound licence is
claimed (the manifest's `license` is
`CC-BY-4.0 AND CC-BY-3.0 AND WordNet AND CC0-1.0`). Its copyright lines still have to
be checked against each project's own licence file before the pack is
published (§6 there).

**Wiktionary is not a source.** It is organised by word, not by meaning; its
senses are not aligned across languages, its structure varies from entry to
entry, and it is share-alike. It is a poor fit for a concept store and is not
used for concepts, glosses or forms.

### The word-pair lexicon

Concept-based translation only works where both languages have a word on the
same concept, and the Spanish wordnet covers about a third of the English one.
A plain bilingual lexicon fills the rest.

- **What it is:** word → candidate translations, best first, in both
  directions; roughly 47,000 English and 55,000 Spanish headwords. It is not
  conceptual: no senses, no relations.
- **Origin:** the lexicon the owner built for Tomos, reduced from the
  `IvanDubls/llm_dictionary` English–Spanish dataset (MIT licence), whose
  entries were generated by a language model, not curated by linguists.
- **Roles.** It is the bridge between the two languages wherever the wordnets
  do not meet, which makes it a cornerstone and not only a fallback:
  1. *Translation fallback* for `translate.word`, after the concept store.
  2. *Extending the Spanish side at pack-build time.* When a Spanish word has
     no concept but its translations are English words that share a synset,
     the builder links the Spanish word to that synset as an induced sense,
     with its own source and a lower confidence. This is how the larger
     English wordnet becomes reachable from Spanish.
  3. *Run-time bridge.* For a Spanish word still without a concept, Talos may
     look up its translation and answer from the English concept, saying that
     it did so.
  4. *Vocabulary for language detection* and for recognising a word of the
     other language inside a sentence ("pon un timer").
  5. *Cross-language query expansion* for the search skill.
- **Limits.** Roles 2 and 3 guess at meaning from word pairs, and the lexicon
  is noisy. The precision of induced senses must be measured on a sample
  before they ship; until then only roles 1, 4 and 5 are assumed. An induced
  or bridged answer is never presented with the same confidence as a curated
  one.
- **Storage:** its own table of word pairs with rank and source, in its own
  pack, outside the concept schema. The lexicon itself holds no concepts;
  induced senses (role 2) are written to the vocabulary pack by the builder,
  under a distinct source.
- **Caveats:** it contains noise (truncated or inflected forms among the
  candidates). It needs a cleaning pass before release, and the pack must
  carry the source's MIT notice.

### Dictionary packs

A dictionary pack is a whole dictionary of one language: for each headword,
its definitions under each part of speech, and its inflected forms. It is
what `define` answers from when one is installed for the conversation's
language; the vocabulary pack stays the source for translation, and for
definitions in a language with no dictionary pack.

It is a pack like the others (same schema, same checks, `talos packs`), told
apart by `role = dictionary` in its manifest:

| Table | Holds |
|---|---|
| `lexeme` | one row per headword and part of speech; `pos` is the dictionary's own name ("sustantivo", "verbo") |
| `sense` | one row per definition, `rank` its position in the entry, `gloss` its text |
| `concept` | one row per definition (`kind = sense`), only so references resolve: a definition is a sense of one word, not a concept shared between languages |
| `form` | the headword (features `NULL`) and the inflected forms the dictionary lists (features `form`), on the headword's first lexeme |

A dictionary pack is attached apart from the concept packs: it is not among
the stores the concept queries ask (`Knowledge::stores`), so sense choice,
relations and translation never see its rows. One query reads it,
`Knowledge::entries(word, lang)`:

- the headwords spelt as typed, without regard to case or accents, the one
  typed letter for letter first ("papá" before "papa");
- else the headwords the word is a listed form of, and the ones the rules
  for nouns and adjectives read it as (a dictionary does not list every
  plural), the shortest first ("fui": "ir", then "ser"; "mejores": "mejor",
  then "mejorar"). A pronominal verb is folded into its verb ("irse" into
  "ir"): they repeat each other's forms.

`define` gives the first headword's entry: three definitions for each of
its first two parts of speech, how many more there are, and the
dictionary's name. Nothing is chosen or asked.

**Making one.** `talos-pack kobo FILE --lang es --name NAME --license
LICENSE [--url URL] [OUT]` converts a dictionary in the format e-readers
use (Kobo's `dicthtml`: a ZIP of gzipped HTML fragments). Definitions are
copied without change of wording; synonym lists, pronunciations and
etymologies are left out; a form listed by an entry of a different number
of words is dropped, because a locution lists every form of its verb. The
conversion needs nothing new in the binary, which reads the result with
SQLite. A hand-written pack source becomes one with `role = "dictionary"`
in `[pack]` (`knowledge/test-dictionary.toml`, for tests).

Talos ships no dictionary and none is in this repository: whoever installs
one chooses it and answers for its licence, which the pack records and
`talos packs info` shows. The name, licence and address are given on the
command line because the file does not carry them.

Measured with the Spanish dictionary built from Wiktionary by reader.dict
(snapshot 2026-11-01, a 12.6 MB file): 111,227 headwords, 176,600
definitions, 636,805 inflected forms; an 80 MB pack (18.5 MB under zstd),
built in 3 s. Half of the pack is the forms and their two indexes. Peak
resident memory of a conversation with fifteen definitions: 9.2 MB with no
pack, 9.5 MB with the dictionary, 9.6 MB with the dictionary and the
vocabulary pack. Of 69 everyday, learned and inflected Spanish words, 69
were answered from it.

Known limits: senses come in the dictionary's order, which is not always
the commonest first ("dinero" gives the old coin first); a plural with an
entry of its own ("casas") is answered by that entry; definitions keep the
dictionary's subject labels ("Mamíferos, perros: …").

### The Spanish gloss gap

The research in [wordnet-sources.md](research/wordnet-sources.md) found that
the open Spanish wordnet is much smaller than the English one (about 38,500
synsets against 120,500) and that only a small minority of its synsets carry a
Spanish definition — second-hand figures suggest 10–18 %, unverified. Relations
and word translation are unaffected; Spanish *definitions* are. Measured on
the files used (wordnet-sources.md §2.3) and in the built vocabulary pack:
78 215 concepts have Spanish words, 18 853 of them (24 %) a Spanish
definition from the MCR; with Wikidata's Spanish descriptions (below) 37 369
(48 %).

The gap is closed in two ways (decided 2026-11-24):

1. **Hand-written glosses for a core subset** — the everyday concepts the
   evaluation battery and ordinary use need. The project's own work, highest
   quality, and the definition Talos prefers whenever one exists.
2. **Build-time translation of the English glosses** for the remainder. Done
   once when the pack is built, never at run time. Each such gloss is stored
   under its own source ("translated from the English wordnet") with a lower
   confidence, so Talos can tell the user a definition is a translation and a
   hand-written one always outranks it. The English glosses' licence permits
   derivatives with attribution, which the pack carries.

Translated glosses are reviewed by sampling before a pack is released, and the
sample's error rate is recorded with the pack. Wikidata's Spanish
descriptions are in the pack since v0.3 as a third source (`wikidata`), for
concepts with no MCR gloss only: short labels, nouns almost only, not
reviewed against the synset (wordnet-sources.md §2.3.2). They do not replace
the two ways above.

Where no Spanish gloss exists, Talos still answers relations and translation
for the concept, and says it has no definition in Spanish rather than showing
the English one unasked.

Inflected forms come from rules plus exception lists maintained in the
project, not from an external dictionary.

## 8. Deliberately left out

- **Embeddings.** Useful later as a ranking signal for sense selection and
  search; not a source of truth, and not in the first major version.
- **Free-text fact extraction.** See §6.
- **Implicit learning from conversation** (word associations, patterns). Not
  wanted: Talos knows what packs contain and what it was explicitly taught.
- **Encyclopedic knowledge** in the first major version. See §7.
- **Probabilistic logic beyond the confidence rules above.**
