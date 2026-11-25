# 0002. Clean-room rewrite and provenance policy

- Status: Accepted
- Date: 2026-11-24

## Context

The Talos that currently lives inside Tomos derives from NOVA AI by Kevin
(Whooptie). That project's licence is "viewable, not reusable": it forbids
copying, forking, derivative works, and replicating its architecture, modules
or data structures for commercial purposes. It also declares several of its
data files to be personal data.

This project keeps the *concept* — a deterministic, semantic, model-free
assistant — which is not anyone's property. It must not carry over the
expression of that concept.

The people and assistants designing this project have read the existing code.
That is a fact to manage, not to hide.

### What the owner added to the earlier Talos

Several ideas central to this design did not come from NOVA AI. They were the
owner's own additions when he repurposed it, and are recorded here so their
origin is not misattributed:

- concepts and relations stored in SQLite (the original kept them in JSON);
- an importer of concepts from external sources;
- removal of the personality and emotion layers;
- the ELIZA-style fallback (the original had no fallback);
- removal of the language-model bridge used to rephrase replies;
- English and Spanish support (the original supported only Dutch);
- integration with Tomos's plugin system.

The new design keeps these ideas because they are the owner's, and drops the
inherited structure they were grafted onto.

## Decision

Talos is written from scratch against a behavioural specification.

1. **No code is copied or translated** from the existing Talos or from NOVA
   AI: no files, functions, identifiers, comments or tests.
2. **No architecture is carried over.** No layer model, no event bus, no
   module layout mirroring the original. The design in this repository is
   justified on its own terms in these records.
3. **No data is carried over** from NOVA AI: no concepts, associations,
   patterns, identity or personality files. Knowledge comes from openly
   licensed sources and from the user.
4. **What may be reused** is material the owner authored independently:
   the intent corpus and evaluation phrases written for Tomos (the owner's
   own, generated with an AI assistant), Tomos's own plugins and their
   behaviour, and public prior art.
5. **Prior art is credited** where it shaped the design: ELIZA (Weizenbaum,
   1966) for the reflector, WordNet and Wikidata for the knowledge model.
6. **Implementation works from the documents in this repository**, not with
   the old source open alongside.

Before reusing any corpus file, check its history for phrases inherited from
the original project and drop them. The owner's audit of his corpus is kept
outside this repository. In the event no corpus file was reused: every
training example here was written anew.

The same test applies to text the owner wrote inside the old code base, such
as his Spanish and English reply wording or fallback patterns: it may be
consulted and reused only where history shows the owner added it and it is not
an adaptation of upstream text. The surrounding code is never carried over.

## Consequences

- Features are specified by behaviour ("answers *is X a Y* with the path"),
  which is slower than porting and produces a better design.
- Some behaviours of the old Talos will be lost or come back differently.
  Parity is a checklist of user-visible capabilities, not of internals.
- The project can choose its own licence.
- This record is a policy, not legal advice. If Talos is published or used
  commercially, the separation deserves a review by someone qualified.

## Alternatives considered

- **Ask the original author for permission.** Still worthwhile as a courtesy,
  and would remove doubt, but leaves the project dependent on someone else's
  terms and on a design it wants to leave behind anyway.
- **Keep refactoring the existing code.** Does not change what it derives
  from.
