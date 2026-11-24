# 0006. Knowledge packs as attachable read-only databases

- Status: Accepted
- Date: 2026-11-24

## Context

Knowledge should travel separately from the program: installable, removable,
shareable, and licensed on its own terms. Useful open sources have different
licences (Wikidata is CC0, Wiktionary is share-alike), and the user's own
taught knowledge must never be mixed irreversibly with imported data.

## Decision

- A **pack** is a single read-only SQLite file (`*.talospack`) using the
  knowledge schema, with name, version, languages, licence and source recorded
  in its `meta` table.
- Packs are **attached** at start-up next to the writable `user.db`. Views
  present them as one store.
- Concept identifiers are **global text keys**, so packs compose without
  renumbering and can refer to each other's concepts.
- `user.db` is an **overlay**: user claims override pack claims at query time;
  retracting a pack claim writes a tombstone in `user.db`. Packs are never
  modified.
- Packs are built by tools in `tools/import/` and distributed separately from
  the binary.

## Consequences

- Installing or removing knowledge is copying or deleting a file.
- A pack's licence obligations stay with the pack.
- The user's knowledge is always separable, exportable and small.
- Queries span several attached databases; cost must be measured. SQLite's
  limit on attached databases caps the number of simultaneous packs.
- Text keys cost more space than integers; acceptable, and measurable.

## Spike result (2026-11-24)

Attaching is fast enough; the fallback (merge packs at install time) is not needed for
latency. On an Apple M4, with `user.db` plus 8 attached packs of a WordNet-sized synthetic
corpus (120k concepts, 255k claims, 200k lexemes), the well-formed queries (lemma lookup,
form to lemma, bounded is_a, kinds-of, part_of, neighbourhood) have p99 of 0.03-0.34 ms
against the 10 ms budget; start-up is 2-3 ms. Estimated Pi factor: 4-8x (an estimate, not a
measurement; cold SD-card reads were not measured). The decision stands, but it only holds
if queries are not written against the views alone:

- A recursive CTE joined to a `UNION ALL` view scans every pack and costs 80-95 ms, even
  with one pack plus the overlay. Queries must be generated per attached database
  (15-45 µs for the same question), and view-to-view joins avoided (N² plans).
- Packs should be attached with `immutable=1`, with a small `cache_size` per attached
  database (each has its own cache: about 2.8 MB per pack by default), and
  `foreign_keys` off (cross-pack references cannot be enforced).
- The bundled SQLite allows at most 10 attached databases unless compiled otherwise.

No contradiction with the decision; the sentence "Views present them as one store" should be
read as "for simple lookups". Recommended refinements for the owner to rule on: record the
three constraints above as part of the decision, and keep "merge at install time" as an
option for many small packs only (merging saves memory and a factor 2-5 on point queries, but
does not remove the need for generated queries). Full method, numbers, query plans and what
was not measured: [pack attachment spike](../research/pack-attachment.md).

## Alternatives considered

- **Merge everything into one database on import.** Fastest queries; loses
  clean removal and blurs provenance. Kept as the fallback above.
- **Ship knowledge inside the binary.** Simple for a tiny core set (and done
  for that), wrong for anything large or separately licensed.
