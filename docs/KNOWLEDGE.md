# Knowledge

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

A pack is a read-only SQLite file with this schema, a manifest row in `meta`
(name, version, languages, licence, source), and an FTS index.

- Packs are attached at start-up, read-only and immutable, each with a small
  page cache of its own. Views present `user.db` and the packs as one store
  **for simple lookups only**.
- Recursive and joining queries are generated per attached database and their
  results combined; they are never written against the union views. The spike
  in [pack-attachment.md](research/pack-attachment.md) measured the same
  is-a question at tens of microseconds per database and at 80–95 ms through
  a view.
- Cross-pack references cannot be enforced by SQLite, so foreign keys are a
  build-time check of the pack builder, not a run-time constraint.
- The bundled SQLite allows ten attached databases, which bounds the number
  of packs in use at once.
- Concept identifiers are global text keys, so packs compose without
  renumbering.
- Packs are distributed separately from the binary. Their licence is their
  own: a pack built from a share-alike source carries that obligation without
  touching the code's licence.

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

### The Spanish gloss gap

The research in [wordnet-sources.md](research/wordnet-sources.md) found that
the open Spanish wordnet is much smaller than the English one (about 38,500
synsets against 120,500) and that only a small minority of its synsets carry a
Spanish definition — second-hand figures suggest 10–18 %, unverified. Relations
and word translation are unaffected; Spanish *definitions* are.

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
sample's error rate is recorded with the pack. Wikidata descriptions remain a
possible supplement and are not part of the plan.

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
