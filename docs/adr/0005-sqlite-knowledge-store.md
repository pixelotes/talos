# 0005. SQLite knowledge store with language-neutral concepts

- Status: Accepted
- Date: 2026-11-24

## Context

The semantic store is the defining feature of Talos. It must hold word
meanings, relations between concepts, provenance and confidence; answer
transitive questions; work in two languages; fit in a few megabytes of memory;
and need no server.

Keeping such data in JSON files loaded into memory costs RAM proportional to
the data and makes every write a rewrite. Tying concepts to the words of one
language makes bilingual support a matter of duplicating everything.

## Decision

Store knowledge in SQLite, with this model (full schema in
[KNOWLEDGE.md](../KNOWLEDGE.md)):

- **Concepts** are language-neutral nodes with stable text identifiers.
- **Lexemes** are words per language; **forms** map inflections to them;
  **senses** link a lexeme to a concept with a gloss.
- **Claims** relate concepts through typed **predicates**, and carry
  confidence, source, validity interval, negation and retraction.
- **Reasoning runs in the database**: transitive closure as bounded recursive
  queries, forward and reverse indexes on claims, FTS5 over glosses.
- Predicate properties (transitive, symmetric, functional, inverse) are data,
  so reasoning rules are not hard-coded per relation.
- WAL mode; one writer, many readers; no write on an ordinary turn.

### Scope of what Talos knows

Out of the box Talos knows **common vocabulary**: what everyday words mean and
how their concepts relate. The backbone for that is the WordNet family, whose
synset — one meaning, with the words that express it — is exactly a concept in
this model, already aligned between Spanish and English and already carrying
hypernym, meronym and antonym relations.

Encyclopedic knowledge (places, people, works) is **not** in the first major
version. It is expected later as an optional pack built from a Wikidata
subset; the model needs no change to take it, since both sources supply
language-neutral identifiers.

Talos learns only what it is explicitly taught. Implicit learning from
conversation (word associations, patterns) is not wanted and is not part of
the design.

## Consequences

- Memory use is independent of knowledge size: SQLite pages what it needs.
- Word translation, cross-language teaching and explicit polysemy fall out of
  the model instead of being features to build.
- Every answer can cite the claims and sources it used.
- Reasoning is limited to what bounded recursive SQL expresses well. That is
  a deliberate ceiling.
- The schema becomes a public format once packs exist; it needs versioning
  from the start.
- Importers must map external data onto concepts. That is direct for
  synset-based and entity-based sources (WordNet, Wikidata).
- **Wiktionary is ruled out as a source** (owner's decision, 2026-11-24): it is
  word-centric, its senses are not aligned across languages, and its entries
  are too irregular to make good concepts.
- The open Spanish wordnet is smaller than the English one and has few Spanish
  definitions (see [wordnet-sources.md](../research/wordnet-sources.md)).
  Spanish glosses therefore come from two origins (decided 2026-11-24):
  hand-written for a core subset, and translated from the English glosses at
  pack-build time for the rest, stored as a distinct lower-confidence source.
  A model is used only to build the pack, never to answer, so
  [0012](0012-no-language-model.md) holds. Details in
  [KNOWLEDGE.md](../KNOWLEDGE.md).

## Implementation notes (v0.3 Slice A, 2026-11-25)

Built as decided, with three settlements recorded in
[DEVELOPMENT.md](../DEVELOPMENT.md#decisions-taken-in-v03-slice-a-the-store):
`rusqlite` with the bundled SQLite, trimmed, behind a `knowledge` feature
(+620 KB, resident memory unchanged); **FTS5 is not compiled in** until
search needs it (the schema adds `gloss_fts` then, by migration); and
`user.db` uses the **rollback journal, not WAL**, because reading a WAL
database creates files, which would be a write on a turn that changes
nothing. One writer and many readers still holds.

## Alternatives considered

- **In-memory graph loaded from files.** Fast, simple, and violates the
  memory budget as soon as the knowledge is interesting.
- **An embedded graph or RDF database.** A heavier dependency for queries that
  recursive CTEs already cover.
- **Word-centric schema (definition per word per language).** Simpler
  importers, but makes bilingual behaviour and reasoning across languages
  awkward forever.
- **Embeddings as the primary store.** Not deterministic in the sense
  required, not explainable, and needs a model at run time.
