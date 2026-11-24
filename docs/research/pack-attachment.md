# Spike: attached knowledge packs versus one merged database

- Date: 2026-11-24
- Question: [ADR 0006](../adr/0006-knowledge-packs.md), "Open point". Packs are read-only
  SQLite files `ATTACH`ed next to a writable `user.db`, presented as one store by
  `UNION ALL` views, with `user.db` as an overlay (user claims override, tombstones retract).
  Is query latency acceptable across several attached databases, or must packs be merged
  into one database at install time?
- Budget: knowledge query p99 <= 10 ms ([DESIGN.md §8](../DESIGN.md)); resident memory
  <= 20 MB with one large pack.
- Code and raw results: [`spikes/pack-attach/`](../../spikes/pack-attach/)
  (`results/` holds every raw TSV and the generated summary; `run_all.sh` reproduces them).

## 1. Answer

**Attach is fast enough. Merging at install time is not required for latency.** On an Apple
M4, with 8 attached packs plus the overlay, the well-formed queries have p99 between 0.03 and
0.34 ms, except two that are expensive in every layout (FTS on common words, 1.7 ms; a
3-level reverse is_a on a hub concept returning ~290 rows, 5 ms).

That holds only if Talos avoids three traps that the "views present them as one store"
wording of the ADR walks into. All three were measured:

1. **A recursive CTE joined to a `UNION ALL` view is catastrophic: 80-95 ms p50, in every
   attached layout, including one pack + overlay.** The planner materialises the whole union
   (every claim of every pack, ~255k rows) and builds an automatic index on it, on every
   execution. The same query on the merged single database is 12 µs. Fix: generate one CTE
   branch per attached database (15-45 µs p50, 28-125 µs p99 at 1/3/8 packs).
2. **A join between two union views (`lexeme_all JOIN sense_all`) plans as the cross product
   of branches: N² probes.** 8 packs: 600 µs; the `IN (subquery)` form is O(N): 152 µs.
3. **Attach packs with `immutable=1`.** Skips per-query lock and change-counter checks on each
   attached file: -13 to -50 % latency at 8 packs and it removes the tail (1-pack lemma p99
   267 µs -> 38 µs).

Two further findings affect the schema and the connection set-up (§7).

So the ADR decision stands; the fallback is not triggered. **Merge is worth doing only for
memory and for packs beyond ~8-10** (SQLite's compile-time cap, §6). Merging does *not*
remove trap 1: layout B (one merged pack + `user.db`) has it too.

## 2. Method

**Machine.** Apple M4 (macOS, 32 GB, NVMe SSD), SQLite 3.50.2 via `rusqlite` 0.37 `bundled`
(FTS5 on, `MAX_ATTACHED=10`, `DEFAULT_FOREIGN_KEYS` on). Release build, single thread, one
process per layout. **This is much faster than the Raspberry Pi target; see §8 for the Pi
factor, which is an estimate and not a measurement.**

**Data (deterministic, seed fixed in `src/data.rs`).** Schema exactly as KNOWLEDGE.md §2
(text concept ids `wn:%08d-n`), plus a `tombstone(claim_id)` table in `user.db`.

| | |
|---|---|
| concepts | 120,000; is_a hierarchy 16 levels (depth 0-15), level sizes peaked at 7-9; parent choice skewed (max fan-out 604, 57,940 non-leaf); 2,402 multi-parent concepts |
| claims | 255,397: is_a 122,401, part_of 90,000, antonym 8,000, causes 5,000, has_property (literal) 30,000 |
| lexemes | 200,000 (120k en, 80k es); 270,182 senses (polysemy 30 % / 25 %); 600,667 forms; glosses of 5-14 words from a log-uniform (Zipf-like) 30k-word vocabulary; Spanish gloss on 60 % of concepts; FTS5 `gloss_fts` over lemma, gloss (contentless) |
| overlay (`user.db`) | 1,000 user claims (400 `user:` concepts with is_a into pack concepts, 200 part_of, 200 overriding is_a, 200 literals) and 199 tombstones (200 requested; one duplicate); lexemes, senses, forms and FTS rows for the user concepts |

**Layouts.** A: one merged database (no overlay; upper bound). B: `user.db` + 1 pack. C:
`user.db` + 3 packs. D: `user.db` + 8 packs. Packs split by sub-tree (concepts under the same
depth-2 node together, balanced) with 15 % of concepts assigned to a random pack, so
**cross-pack references are frequent**: is_a edges crossing packs 19 % (C) / 25 % (D),
part_of 66 % / 87 %. Claims live in the pack of their subject; lexemes, forms, senses and FTS
rows in the pack of the lexeme's first concept. Ids are globally disjoint integers. Views
(`concept_all`, `lexeme_all`, `form_all`, `sense_all`, `claim_all`) are `TEMP` views created
after the attaches; `claim_all` also filters `retracted_at IS NULL` and anti-joins the
tombstones on `main.tombstone` (primary-key probe). FTS cannot go through a view, so the
search statement is a generated `UNION ALL` of one `MATCH` per database.

**Workload.** 3,000 samples per query kind, fixed in `data/samples.tsv`, cycled; 3,000
timed runs after 300 warm-up runs (page cache and statement cache warm). Latency is the
wall time of prepare-from-cache, execute and fetching all rows. Percentiles by sorting the
samples. Queries: lemma → senses (5 % misses); form → lemma; "is X a Y" ≤ 6 hops (50 %
positive, 50 % negative, negatives being the worst case: they exhaust the ancestors);
deep "is X a Y" (X at depth ≥ 13, bound 16); the same from a user concept (hop 1 in
`user.db`, then into packs); kinds-of one level and three levels, for uniformly sampled
non-leaf concepts and for the top-100 "hub" concepts; part_of forward and reverse;
neighbourhood (outgoing claims, incoming claims, words of the concept); FTS top-20 ranked,
1-2 terms (the 40 most common words are excluded as stop words), optionally followed by
resolving 10 hits; teach (claim + audit row, one transaction).

**Variants and checks.** Where a query crosses a view join or a recursion, every variant was
run and compared with the others on 60 samples (10 for hubs): **0 mismatches in all 87
comparisons**.

**Caveats on what "cold" means here.** `purge` needs sudo, which was not available, so
the true cold OS page cache was **not** measured. "Connection-cold" = a brand-new connection
(empty SQLite page cache and statement cache) per run, timing prepare plus first execution,
with the files still in the OS cache. Runs: 2,000.

Where a variant was pathologically slow (> 20 ms/run) the run count was capped by a 60 s
budget, giving 540-750 runs rather than 3,000 (all in the "view CTE" rows; the p99 of ~650
samples is still ~6 observations).

## 3. Results: warm latency (p50 / p99)

Recommended form of each query. A = merged DB; B/C/D = 1/3/8 packs + overlay;
default settings (no `immutable`).

| Query | A | B | C | D |
|---|---|---|---|---|
| lemma → senses (`IN` subquery form) | 18 / 95 µs | 29 / 32 µs | 58 / 66 µs | 152 / 183 µs |
| form → lemma (`IN` form) | 13 / 39 µs | 22 / 25 µs | 45 / 55 µs | 119 / 159 µs |
| is X a Y, ≤ 6 hops (A: view CTE; B-D: per-db branches) | 12 / 25 µs | 16 / 28 µs | 23 / 38 µs | 45 / 70 µs |
| is X a Y, deep (≤ 16) | 22 / 53 µs | 28 / 64 µs | 38 / 112 µs | 65 / 125 µs |
| is X a Y, X a user concept | - | 10 / 17 µs | 18 / 62 µs | 34 / 44 µs |
| kinds-of, 1 level (view, 2 rows avg) | 9 / 24 µs | 18 / 31 µs | 37 / 124 µs | 75 / 85 µs |
| kinds-of, 1 level, hub (67 rows) | 38 / 179 µs | 43 / 168 µs | 49 / 116 µs | 91 / 151 µs |
| kinds-of, 3 levels (5 rows avg) | 12 / 160 µs | 13 / 108 µs | 19 / 114 µs | 38 / 179 µs |
| kinds-of, 3 levels, hub (289 rows) | 286 µs / 3.6 ms | 290 µs / 3.2 ms | 368 µs / 4.2 ms | 479 µs / 5.2 ms |
| part_of forward (X is part of?) | 9.5 / 12.5 µs | 20 / 31 µs | 37 / 47 µs | 83 / 93 µs |
| part_of reverse (parts of X) | 8.8 / 33 µs | 18 / 45 µs | 36 / 64 µs | 77 / 92 µs |
| neighbourhood (`IN` form) | 35 / 93 µs | 66 / 120 µs | 137 / 459 µs | 310 / 338 µs |
| FTS top-20 (ids) | 46 µs / 1.6 ms | 65 µs / 1.5 ms | 105 µs / 1.6 ms | 192 µs / 1.7 ms |
| FTS top-20 + resolve 10 through views | 66 µs / 1.7 ms | 111 µs / 1.6 ms | 232 µs / 2.0 ms | 383 µs / 2.1 ms |
| teach write, `synchronous=NORMAL` (see caveat) | 29 µs / 1.1 ms | 29 / 63 µs | 35 / 82 µs | 48 / 90 µs |
| teach write, `synchronous=FULL` | 50 µs / 3.4 ms | 49 / 91 µs | 54 / 93 µs | 68 / 204 µs |

Reading it:

- Cost grows roughly linearly with the number of attached databases (one index probe per
  database for each point query): about 8 µs per extra database for a view point lookup.
  At 8 packs that is still two orders of magnitude under the 10 ms budget.
- FTS is the slow query, but it is slow in A too: the cost is ranking long posting lists of
  common words, not attachment. Its p99 (1.6-1.7 ms) is independent of layout.
- Teaching is unaffected by the number of packs (writes touch only `user.db`; in A they
  touch the big merged file, hence A's long p99 tail, probably WAL checkpoints; not verified).
  **Caveat:** macOS `fsync` does not flush the drive's cache, so the `FULL` row shows
  syscall cost, not durable-write cost. Real write latency on an SD card is unmeasured.

### 3.1 Recursive queries through the four mechanisms

Warm p50 / p99. "view" = CTE joined to `claim_all` (what the ADR's wording implies).
"branch" = one recursive branch per attached database, generated by Talos. "edge" = a TEMP
table `edge(subject, predicate, object)` of is_a + part_of claims built at start-up with two
indexes. "rust" = bounded BFS in Rust, one prepared statement per database, merging in
application code. Number of runs in parentheses where < 3,000.

| Query | variant | A | B | C | D |
|---|---|---|---|---|---|
| is X a Y ≤ 6 | view | 12 / 25 µs | **81 ms** / 110 ms (744) | **87 ms** / 93 ms (683) | **90 ms** / 92 ms (665) |
| | branch | - | 16 / 28 µs | 23 / 38 µs | 45 / 70 µs |
| | edge | 8.5 / 17 µs | 8.7 / 18 µs | 6.6 / 13 µs | 6.5 / 12 µs |
| | rust | 47 / 132 µs | 109 / 205 µs | 240 / 452 µs | 580 µs / 1.1 ms |
| is X a Y deep ≤ 16 | view | 22 / 53 µs | **82 ms** / 104 ms (735) | **90 ms** / 124 ms (684) | **89 ms** / 115 ms (666) |
| | branch | - | 28 / 64 µs | 38 / 112 µs | 65 / 125 µs |
| | edge | 16 / 39 µs | 17 / 45 µs | 12 / 30 µs | 12 / 29 µs |
| | rust | 114 / 343 µs | 264 / 665 µs | 587 µs / 1.7 ms | 1.4 / 3.3 ms |
| is X a Y ≤ 6, user concept | view | - | **84 ms** / 99 ms (719) | **91 ms** / 156 ms (670) | **88 ms** / 113 ms (682) |
| | branch | - | 10 / 17 µs | 18 / 62 µs | 34 / 44 µs |
| | edge | - | 4.8 / 11 µs | 3.8 / 6.3 µs | 3.9 / 6.5 µs |
| kinds-of 3 levels | view | 12 / 160 µs | **93 ms** / 112 ms (633) | **98 ms** / 136 ms (539) | **92 ms** / 95 ms (648) |
| | branch | - | 13 / 108 µs | 19 / 114 µs | 38 / 179 µs |
| | edge | 6.9 / 81 µs | 7.4 / 87 µs | 5.8 / 62 µs | 5.5 / 54 µs |
| | rust | 25 / 318 µs | 56 / 651 µs | 120 µs / 1.4 ms | 283 µs / 3.2 ms |
| kinds-of 3 levels, hub | view | 286 µs / 3.6 ms | **94 ms** / 100 ms (647) | **95 ms** / 110 ms (627) | **93 ms** / 101 ms (628) |
| | branch | - | 290 µs / 3.2 ms | 368 µs / 4.2 ms | 479 µs / 5.2 ms |
| | edge | 230 µs / 2.6 ms | 252 µs / 3.1 ms | 188 µs / 2.8 ms | 152 µs / 2.2 ms |
| | rust | 982 µs / 8.6 ms | 2.3 / 14 ms | 5.0 / 44 ms (2,228) | 11 / 66 ms (1,155) |

Costs of the alternatives:

- **branch**: no start-up cost, no extra memory, no invalidation. Needs SQL generated from the
  list of attached databases (a few lines; it is what `cte_sql` in `src/bench.rs` does).
  Requires SQLite ≥ 3.34 for multiple recursive terms (the bundled one is 3.50).
- **edge**: fastest at query time (3-10x faster than branch at 8 packs) but **165-180 ms to
  build at start-up** (213,002 rows, M4, warm OS cache), extra resident memory (peaks in the full runs rose by 2-6 MB, confounded by
  the other variants: 23.8 vs 17.5 MB at A, 27.8 vs 25.8 at B, 52.3 vs 50.4 at D) and must be
  maintained on every teach/retract. On the Pi the build would cost around a second, ten times
  the 100 ms cold-start budget. Not recommended; it would only pay for a persistent per-pack
  edge index built at pack-build time, which the per-database branch makes unnecessary.
- **rust**: 3-30x slower than branch, with a long tail on hubs (p99 66 ms at 8 packs). No
  advantage. Not recommended.

## 4. What the query plans showed

Plans are in `spikes/pack-attach/results/plans_{A,B,C,D}.tsv` (`EXPLAIN QUERY PLAN`).

**Point queries through the views use the indexes.** `SELECT subject FROM claim_all WHERE
object=? AND predicate=1 …` plans as a `COMPOUND QUERY` whose every branch is `SEARCH c USING
INDEX claim_reverse (object=? AND predicate=?)`; the tombstone test shows as `CORRELATED
SCALAR SUBQUERY … SEARCH t USING INTEGER PRIMARY KEY (rowid=?)`. The same for
`claim_forward`, `sense_concept`, `sense_lexeme`, `form_lookup`, and the lexeme unique index
(`SEARCH p1.lexeme USING COVERING INDEX sqlite_autoindex_lexeme_1 (lang=? AND lemma=?)`).
Push-down works because the constant/parameter terms are copied into each branch.

**The view CTE does not.** For `WITH RECURSIVE r … JOIN claim_all cl ON cl.subject=r.c`:

```
CO-ROUTINE claim_all            -- the union is evaluated as a whole
  COMPOUND QUERY
    LEFT-MOST SUBQUERY   SCAN c            -- full scan of pack 1's claims
      CORRELATED SCALAR SUBQUERY -> SEARCH t USING INTEGER PRIMARY KEY
    UNION ALL            SCAN c            -- ... and so on for every pack and user.db
SCAN r
SEARCH cl USING AUTOMATIC PARTIAL COVERING INDEX (subject=? AND predicate=? AND negated=?)
```

The join term `cl.subject = r.c` references the recursion table, so it cannot be pushed into
the union branches; SQLite scans every branch and builds an automatic index over the result
for each execution. It is the same pattern for 1, 3 or 8 packs (the full 255k claims are
scanned either way), hence ~90 ms everywhere. On layout A the view is a single `SELECT …
WHERE retracted_at IS NULL`, which flattens into the base table, and the plan is the ideal
`SEARCH c USING INDEX claim_forward (subject=? AND predicate=?)`. With generated per-database
branches the plan is a `COMPOUND QUERY` (`SCAN r` + `SEARCH cl USING INDEX claim_forward`
per database, with the tombstone probe), which is what the 45 µs in layout D buys.

**View-to-view joins explode.** `lexeme_all l JOIN sense_all s ON s.lexeme_id=l.id` plans as
nested `MERGE (UNION ALL)` of `LEFT`/`RIGHT` pairs: for B (two branches in each view) there
are four combinations (`p1.lexeme`×`p1.sense`, `p1.lexeme`×`main.sense`, `main.lexeme`×…),
for D 9×9 = 81 sets of index probes, each with its own `USE TEMP B-TREE FOR ORDER BY`. The
`IN (subquery)` form plans as one `LIST SUBQUERY` of N lexeme probes followed by N sense probes.
Measured at D: join 599 µs p50 vs `IN` 152 µs; neighbourhood 491 vs 310 µs. The cost of
preparing the statement shows the same: 408 µs vs 100 µs at D (§5).

**FTS.** Per database, `SCAN p1.gloss_fts VIRTUAL TABLE INDEX 32:M2` in a co-routine, then a
temporary sort; the outer query merges the per-database top-20 by `rank`. Note that bm25
scores computed per pack use per-pack document statistics, so merged ranking across packs is
only approximately comparable (a correctness question, not measured).

## 5. Sizes, memory, start-up

**Disk (bytes on disk, same data).** Splitting costs 7.5-12.7 % (per-file b-tree/index
overhead, FTS segments, statistics, duplicated predicate and source rows):

| layout | files | total |
|---|---|---|
| A merged | 1 | 128.9 MB |
| B | pack 128.8 MB + `user.db` 0.4 MB | 129.3 MB |
| C | 3 packs of 46.1-46.3 MB + `user.db` | 138.9 MB |
| D | 8 packs of 17.3-23.4 MB + `user.db` | 145.2 MB |

Inside one pack: `sense` 26.8 MB (the gloss is copied into every sense), `form` 15.0,
`claim` 14.2, `gloss_fts` 13.9, `form_lookup` 10.8, `claim_forward` 5.7, `claim_reverse`
5.4, `sense_concept` 5.7, lexeme unique index 4.8, `concept` 3.2. The three claim structures
total about 25 MB for 255k claims (~100 B/claim): the text-key cost named in the ADR. Whether
integer keys would shrink this materially was not measured.

**Peak resident memory of the process** (`getrusage`, one process per layout; includes the
~7.4 MB baseline of the harness itself, which loads the sample lists; "after open" is
8.9-9.8 MB for all layouts):

| layout | default per-db cache (2 MB) | per-db cache 256 KiB, `immutable=1` | `mmap_size`=256 MB |
|---|---|---|---|
| A | 16.5 MB | 14.4 MB | 98 MB |
| B | 19.0 MB | - | 100 MB |
| C | 24.4 MB | - | 103 MB |
| D | 39.0 MB | **21.9** MB (512 KiB: 23.7; 1 MiB: 28.7) | 107 MB |

(This is the mixed "quick" workload without the slow view CTEs. Over the full variant suite,
which includes the full scans of §3.1, peaks were 17.5 / 25.8 / 36.5 / 50.4 MB.)

Every attached database has its **own** page cache (`PRAGMA cache_size` applies per schema):
about 2.8 MB per attached pack at the default. It must be set explicitly per attached
database, and a pool of reader connections multiplies it. With 256 KiB per database, latency
was unchanged at D (lemma 137 µs vs 134 µs; kinds-of 1 level 60 vs 57 µs) because the
files sit in the OS cache; on a Pi with a cold card this trade is **unmeasured**.
`mmap` makes resident size balloon (mapped pages count) and gained 3-12 %. Not worth it.

**Start-up** (M4, OS cache warm, 300 repetitions, p50 / p99):

| layout | open + attach + create views | + first lemma query (cumulative) |
|---|---|---|
| A | 0.67 / 2.6 ms | 1.6 / 3.3 ms |
| B | 0.77 / 1.3 ms | 1.9 / 2.1 ms |
| C | 1.2 / 1.8 ms | 2.1 / 2.4 ms |
| D | 2.1 / 2.8 ms | 2.9 / 3.2 ms |

Statement preparation grows with N, because every view expands into N branches:
`lemma → senses` (join) 3.8 / 16.5 / 82 / 408 µs (A/B/C/D), `IN` form 3.8 / 10.5 / 28 / 100 µs,
simple point view 2 / 4.5 / 14 / 37 µs. Prepare statements once and keep them.

**Connection-cold** (new connection per run, includes prepare), p50 / p99: lemma → senses
(join) 36 µs / 0.47 ms (A), 84 / 135 µs (B), 243 / 389 µs (C), 1.0 / 1.1 ms (D); FTS 94 µs /
1.9 ms (A) to 432 µs / 2.4 ms (D). Everything except the view CTE is ≤ 2.4 ms p99.

## 6. Settings that mattered (and did not)

Quick workload, layout D unless stated (p50 / p99). Baseline: default cache, no mmap, WAL.

| Setting | Effect |
|---|---|
| `ATTACH 'file:…?mode=ro&immutable=1'` | **Mattered most.** D: is_a 46 -> 23 µs p50, kinds-of 79 -> 57, lemma 154 -> 134, neighbourhood 318 -> 253. B: lemma p99 267 -> 38 µs, neighbourhood p99 326 -> 76. Removes lock/change-counter syscalls per attached file per statement. Safe only because packs are never modified while open (replace = detach and reattach). |
| `cache_size` per database | No latency effect between 256 KiB and 64 MiB with a warm OS cache; **memory effect is large** (D peak 22 MB at 256 KiB, 101 MB at 8 MiB, 120 MB at 64 MiB). |
| `mmap_size` 256 MB | -3 to -12 % latency; +60-70 MB resident. No. |
| WAL vs rollback journal on `user.db` | WAL slightly better (reads 2-4 %); keep WAL. Packs are not in WAL mode (read-only, `journal_mode=DELETE`). |
| `synchronous` NORMAL vs FULL | Only affects the teach write: 29 vs 50 µs p50 on this Mac (see caveat in §3). |
| `foreign_keys` | **Must be OFF.** The bundled SQLite compiles with `DEFAULT_FOREIGN_KEYS`; a claim whose subject is in another pack fails with `FOREIGN KEY constraint failed` (hit while building the packs). Cross-database foreign keys cannot exist. The `REFERENCES` in KNOWLEDGE.md §2 are documentation only. |
| `MAX_ATTACHED` | **10** in the bundled build (accepts exactly 10 attaches; `user.db` is `main`, so 10 packs max). Raising it needs a compile flag (`LIBSQLITE3_FLAGS` for `libsqlite3-sys`, up to 125). 8 attached packs behave fine. |

## 7. Changes the schema/design needs whichever layout wins

- KNOWLEDGE.md §2 lists no index on `sense`. Lemma → senses and concept → words need
  `sense(lexeme_id)` and `sense(concept_id)`; the spike added both (about 8.7 MB per 270k senses).
- Views must be created `TEMP` after the attaches (a persistent view in `user.db` cannot
  depend on attached schemas), so they are rebuilt at every start (part of the 0.7-2.1 ms open time in §5).
- The `claim_all` view hard-codes the overlay: `NOT EXISTS (SELECT 1 FROM main.tombstone t
  WHERE t.claim_id = c.id)` per pack branch. Cost measured as part of B-D (one PK probe per
  candidate row; invisible in the numbers above). Overrides were modelled as tombstone + new
  user claim; `valid_from/valid_to` filtering and "negated user claim hides a pack triple
  with the same (s, p, o)" were **not** modelled (the second would need a key-based anti-join
  and is the main thing that could make the overlay costlier).
- The query layer must be generated from the list of attached databases (CTE branches,
  FTS union), not written against views.

## 8. Raspberry Pi factor (estimate, not measured)

Nothing here ran on a Raspberry Pi. **My estimate**: a Pi 4 or Pi 5 is about 4-8x slower than
this M4 for warm, CPU-bound SQLite point queries (lower clock, narrower core, smaller caches;
the larger end for a Pi 4/Cortex-A72). Cold start and any page that misses the OS cache will
cost far more on an SD card (hundreds of µs to ms per random 4 KiB read), a factor this spike
did not measure and which could dominate.

Applying 4-8x to the recommended design (attach, `immutable`, per-database branches), D
(8 packs):

| Query | M4 p99 | Pi p99, estimated |
|---|---|---|
| lemma / form / part_of / kinds-of 1 level / neighbourhood | 0.09-0.34 ms | 0.4-2.7 ms |
| is X a Y, deep | 0.13 ms | 0.5-1.0 ms |
| kinds-of 3 levels | 0.18 ms | 0.7-1.4 ms |
| open + attach + views + first answer | 3.2 ms | 13-26 ms |
| FTS top-20 (common words), any layout | 1.7 ms | **7-14 ms** |
| kinds-of 3 levels on a hub, 289 rows | 5.2 ms | **21-42 ms** |

Against p99 <= 10 ms: attach is comfortable for the symbolic lookups and reasoning. The two
exceptions are not caused by attachment (they cost about the same in the merged DB: 1.6 ms
and 3.6 ms) and are a result-size and ranking question: cap rows (`LIMIT`), cap FTS to
the top terms or paginate, and avoid the unlimited hub expansion. With the view CTE (trap 1)
the same query would cost 0.4-0.7 s on the Pi, over the budget by a factor of 40-70.

Memory against 20 MB at rest with one large pack: at the default cache size D (39 MB peak
including 7.4 MB harness) is over, B-like layouts (19 MB) are on the edge, and D with 256 KiB
caches (21.9 MB) is close; a real binary will not carry the harness baseline. Packs are best
kept small in number or cache-limited.

## 9. Recommendation

**Attach**, as ADR 0006 decided, with the constraints in §1 and §6 written into the design:

1. Generate all multi-database SQL from the list of attached packs; never recurse through a
   `UNION ALL` view and never join two union views (use `IN (subquery)` or two statements).
2. Attach packs read-only with `immutable=1`; set `cache_size` explicitly per database
   (about 256-512 KiB); `foreign_keys=OFF`; no mmap.
3. Pack count: up to 8 is fine (D); more than 10 needs a SQLite compile flag, and beyond
   ~8 the per-pack costs (N probes per point query, N caches, N-branch statements) argue for
   merging.
4. **Hybrid, when needed:** "merge at install time" stays a valid fallback for many small
   packs, but note that a merged pack + `user.db` (layout B) still has the view-CTE trap, so
   the generated-branch queries are needed regardless. Merging buys a factor 2-5 on point
   queries and about 20 MB of memory, not correctness or the budget.
5. If is_a/part_of reasoning ever needs to be faster than 45 µs (it does not today), the
   edge table is a measurable 3-10x, at 170 ms of start-up on a fast machine; it would
   have to be built into the pack file instead.

## 10. Not measured

- **A Raspberry Pi**, and the true cold OS cache (needs `sudo purge`; an SD card or a Pi
  USB SSD). Pi factor in §8 is an estimate.
- **Real data shape.** The data is synthetic: real WordNet has different fan-out
  correlation, lemma and gloss distributions, many more multiword forms, a Spanish side with
  its own coverage, and a different cross-pack structure (a real split is likely by language
  or by source, not by random sub-tree).
- **Concurrency.** A pool of reader connections (each with N page caches), readers during a
  teach write, WAL checkpoint behaviour, and attaching/detaching a pack at run time.
- **Durable write latency** (macOS `fsync`; Linux ext4 on SD).
- **Overlay semantics beyond tombstone-by-id**: temporal validity, user negation of a pack
  triple, per-(s,p) functional override, and the cost of a large overlay (1,000 claims here).
- **Ranking across packs** for FTS and for polysemy, which needs per-pack statistics to be
  merged, and duplicate lexemes (same lemma in two packs).
- **Integer vs text keys**, page size, `WITHOUT ROWID` for `concept`, the cost of omitted
  `ANALYZE` statistics (packs ship with `ANALYZE` and FTS `optimize` here).
- **More than 8 packs** and the compile-time attach limit.
- **Process-wide effects**: allocator behaviour, 32-bit builds, a different libc.
