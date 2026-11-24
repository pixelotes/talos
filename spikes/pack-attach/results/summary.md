
### Warm (all variants)

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 | runs (A/B/C/D) |
|---|---|---|---|---|---|
| lemma->senses / join | 18.6 µs / 323 µs | 41.2 µs / 276 µs | 131 µs / 326 µs | 599 µs / 832 µs | 3000/3000/3000/3000 |
| lemma->senses / in-subquery | 17.9 µs / 95.1 µs | 28.5 µs / 31.8 µs | 57.8 µs / 65.6 µs | 152 µs / 183 µs | 3000/3000/3000/3000 |
| form->lemma / join | 12.3 µs / 132 µs | 27.2 µs / 252 µs | 79.2 µs / 397 µs | 334 µs / 521 µs | 3000/3000/3000/3000 |
| form->lemma / in-subquery | 13.2 µs / 39.2 µs | 21.8 µs / 25.0 µs | 45.3 µs / 55.2 µs | 119 µs / 159 µs | 3000/3000/3000/3000 |
| isa<=6 / view | 11.9 µs / 25.0 µs | 81.4 ms / 109.8 ms | 87.1 ms / 93.4 ms | 89.5 ms / 92.3 ms | 3000/744/683/665 |
| isa<=6 / rust | 46.8 µs / 132 µs | 109 µs / 205 µs | 240 µs / 452 µs | 580 µs / 1.1 ms | 3000/3000/3000/3000 |
| isa deep(<=16) / view | 22.0 µs / 53.4 µs | 81.9 ms / 103.5 ms | 89.7 ms / 123.5 ms | 89.0 ms / 115.2 ms | 3000/735/684/666 |
| isa deep(<=16) / rust | 114 µs / 343 µs | 264 µs / 665 µs | 587 µs / 1.7 ms | 1.4 ms / 3.3 ms | 3000/3000/3000/3000 |
| kinds-of 1 level / view | 9.2 µs / 23.6 µs | 17.9 µs / 30.5 µs | 37.0 µs / 124 µs | 74.7 µs / 85.4 µs | 3000/3000/3000/3000 |
| kinds-of 1 level (hub) / view | 38.4 µs / 179 µs | 43.0 µs / 168 µs | 48.5 µs / 116 µs | 90.5 µs / 151 µs | 3000/3000/3000/3000 |
| kinds-of 3 levels / view | 12.2 µs / 160 µs | 93.1 ms / 111.7 ms | 98.4 ms / 136.2 ms | 92.2 ms / 95.0 ms | 3000/633/539/648 |
| kinds-of 3 levels / rust | 25.4 µs / 318 µs | 55.6 µs / 651 µs | 120 µs / 1.4 ms | 283 µs / 3.2 ms | 3000/3000/3000/3000 |
| kinds-of 3 levels (hub) / view | 286 µs / 3.6 ms | 93.5 ms / 100.0 ms | 94.5 ms / 110.3 ms | 92.5 ms / 100.8 ms | 3000/647/627/628 |
| kinds-of 3 levels (hub) / rust | 982 µs / 8.6 ms | 2.3 ms / 14.1 ms | 5.0 ms / 43.6 ms | 11.2 ms / 65.9 ms | 3000/3000/2228/1155 |
| part_of forward (X is part of?) / view | 9.5 µs / 12.5 µs | 19.6 µs / 30.7 µs | 37.1 µs / 47.1 µs | 83.1 µs / 92.8 µs | 3000/3000/3000/3000 |
| part_of reverse (parts of X) / view | 8.8 µs / 33.0 µs | 18.1 µs / 45.3 µs | 35.5 µs / 63.8 µs | 77.4 µs / 92.3 µs | 3000/3000/3000/3000 |
| neighbourhood / join | 33.0 µs / 43.2 µs | 71.2 µs / 297 µs | 164 µs / 352 µs | 491 µs / 672 µs | 3000/3000/3000/3000 |
| neighbourhood / in-subquery | 35.2 µs / 93.3 µs | 66.0 µs / 120 µs | 137 µs / 459 µs | 310 µs / 338 µs | 3000/3000/3000/3000 |
| fts gloss top-20 / ids | 45.9 µs / 1.6 ms | 65.4 µs / 1.5 ms | 105 µs / 1.6 ms | 192 µs / 1.7 ms | 3000/3000/3000/3000 |
| fts gloss top-20 + resolve 10 / ids+view | 66.4 µs / 1.7 ms | 111 µs / 1.6 ms | 232 µs / 2.0 ms | 383 µs / 2.1 ms | 3000/3000/3000/3000 |
| isa<=6 / branch | - | 15.6 µs / 27.8 µs | 22.8 µs / 37.8 µs | 45.0 µs / 70.4 µs | -/3000/3000/3000 |
| isa deep(<=16) / branch | - | 28.3 µs / 63.8 µs | 37.5 µs / 112 µs | 65.0 µs / 125 µs | -/3000/3000/3000 |
| isa<=6 (user concept) / view | - | 84.2 ms / 98.7 ms | 90.9 ms / 155.8 ms | 87.5 ms / 112.9 ms | -/719/670/682 |
| isa<=6 (user concept) / branch | - | 10.2 µs / 17.1 µs | 17.5 µs / 62.0 µs | 33.5 µs / 43.5 µs | -/3000/3000/3000 |
| isa<=6 (user concept) / rust | - | 49.5 µs / 105 µs | 105 µs / 321 µs | 228 µs / 502 µs | -/3000/3000/3000 |
| kinds-of 3 levels / branch | - | 12.5 µs / 108 µs | 19.0 µs / 114 µs | 37.5 µs / 179 µs | -/3000/3000/3000 |
| kinds-of 3 levels (hub) / branch | - | 290 µs / 3.2 ms | 368 µs / 4.2 ms | 479 µs / 5.2 ms | -/3000/3000/3000 |

### Edge-table variant (TEMP table of is_a/part_of edges)

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 | runs (A/B/C/D) |
|---|---|---|---|---|---|
| isa<=6 / edge | 8.5 µs / 16.7 µs | 8.7 µs / 18.0 µs | 6.6 µs / 12.7 µs | 6.5 µs / 12.2 µs | 3000/3000/3000/3000 |
| isa deep(<=16) / edge | 16.3 µs / 39.2 µs | 17.0 µs / 44.6 µs | 12.1 µs / 30.1 µs | 12.0 µs / 29.1 µs | 3000/3000/3000/3000 |
| kinds-of 3 levels / edge | 6.9 µs / 80.6 µs | 7.4 µs / 86.7 µs | 5.8 µs / 61.7 µs | 5.5 µs / 54.0 µs | 3000/3000/3000/3000 |
| kinds-of 3 levels (hub) / edge | 230 µs / 2.6 ms | 252 µs / 3.1 ms | 188 µs / 2.8 ms | 152 µs / 2.2 ms | 3000/3000/3000/3000 |
| isa<=6 (user concept) / edge | - | 4.8 µs / 10.7 µs | 3.8 µs / 6.3 µs | 3.9 µs / 6.5 µs | -/3000/3000/3000 |

### Connection-cold (fresh connection each run; includes prepare)

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 | runs (A/B/C/D) |
|---|---|---|---|---|---|
| lemma->senses / join | 35.9 µs / 473 µs | 84.1 µs / 135 µs | 243 µs / 389 µs | 1.0 ms / 1.1 ms | 2000/2000/2000/2000 |
| form->lemma / join | 30.2 µs / 137 µs | 59.5 µs / 90.7 µs | 159 µs / 390 µs | 640 µs / 672 µs | 2000/2000/2000/2000 |
| isa<=6 / view | 136 µs / 236 µs | 82.2 ms / 96.5 ms | 86.8 ms / 111.1 ms | 91.1 ms / 108.1 ms | 2000/2000/2000/2000 |
| kinds-of 1 level / view | 75.5 µs / 148 µs | 42.2 µs / 61.2 µs | 81.1 µs / 178 µs | 177 µs / 224 µs | 2000/2000/2000/2000 |
| part_of reverse (parts of X) / view | 29.0 µs / 129 µs | 44.5 µs / 99.2 µs | 90.8 µs / 314 µs | 178 µs / 238 µs | 2000/2000/2000/2000 |
| neighbourhood / join | 76.8 µs / 128 µs | 137 µs / 171 µs | 316 µs / 625 µs | 969 µs / 1.1 ms | 2000/2000/2000/2000 |
| fts gloss top-20 / ids | 93.7 µs / 1.9 ms | 118 µs / 1.9 ms | 234 µs / 2.2 ms | 432 µs / 2.4 ms | 2000/2000/2000/2000 |

### Statement preparation (planning) cost, not cached

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 |
|---|---|---|---|---|
| lemma->senses / join | 3.8 µs / 4.8 µs | 16.5 µs / 30.0 µs | 82.0 µs / 102 µs | 408 µs / 473 µs |
| lemma->senses / in-subquery | 3.8 µs / 4.5 µs | 10.5 µs / 11.4 µs | 27.7 µs / 34.0 µs | 100 µs / 126 µs |
| form->lemma / join | 2.8 µs / 3.4 µs | 8.5 µs / 12.6 µs | 46.0 µs / 55.7 µs | 252 µs / 292 µs |
| form->lemma / in-subquery | 2.6 µs / 3.2 µs | 6.0 µs / 10.9 µs | 19.4 µs / 25.2 µs | 72.5 µs / 90.8 µs |
| isa<=6 / view | 7.4 µs / 9.0 µs | 14.2 µs / 15.5 µs | 24.5 µs / 29.9 µs | 47.4 µs / 67.2 µs |
| isa deep(<=16) / view | 7.4 µs / 9.1 µs | 14.2 µs / 16.7 µs | 24.5 µs / 27.9 µs | 47.8 µs / 60.3 µs |
| kinds-of 1 level / view | 2.1 µs / 7.9 µs | 4.5 µs / 5.6 µs | 13.9 µs / 15.6 µs | 37.2 µs / 47.3 µs |
| kinds-of 1 level (hub) / view | 2.0 µs / 2.1 µs | 4.5 µs / 5.8 µs | 13.8 µs / 15.5 µs | 37.2 µs / 51.7 µs |
| kinds-of 3 levels / view | 7.5 µs / 12.8 µs | 14.4 µs / 14.9 µs | 23.8 µs / 29.3 µs | 46.1 µs / 61.1 µs |
| kinds-of 3 levels (hub) / view | 7.4 µs / 9.1 µs | 14.9 µs / 16.5 µs | 24.2 µs / 26.5 µs | 46.7 µs / 55.8 µs |
| part_of forward (X is part of?) / view | 2.5 µs / 2.7 µs | 5.4 µs / 6.8 µs | 15.9 µs / 20.4 µs | 41.6 µs / 49.1 µs |
| part_of reverse (parts of X) / view | 2.0 µs / 2.5 µs | 4.6 µs / 5.1 µs | 13.9 µs / 14.1 µs | 37.2 µs / 49.6 µs |
| neighbourhood / join | 6.8 µs / 7.1 µs | 17.0 µs / 21.8 µs | 73.2 µs / 84.4 µs | 323 µs / 370 µs |
| neighbourhood / in-subquery | 6.6 µs / 8.2 µs | 16.3 µs / 17.8 µs | 50.5 µs / 60.0 µs | 171 µs / 194 µs |
| fts gloss top-20 / ids | 2.7 µs / 3.2 µs | 4.5 µs / 7.2 µs | 11.4 µs / 11.8 µs | 27.3 µs / 33.9 µs |
| fts gloss top-20 + resolve 10 / ids+view | 3.9 µs / 4.3 µs | 6.4 µs / 7.2 µs | 15.2 µs / 20.3 µs | 40.6 µs / 55.6 µs |
| isa<=6 / branch | - | 16.8 µs / 27.6 µs | 37.4 µs / 47.2 µs | 89.0 µs / 106 µs |
| isa deep(<=16) / branch | - | 16.6 µs / 21.1 µs | 37.2 µs / 44.0 µs | 89.0 µs / 101 µs |
| isa<=6 (user concept) / view | - | 14.2 µs / 17.2 µs | 24.5 µs / 25.0 µs | 47.5 µs / 67.0 µs |
| isa<=6 (user concept) / branch | - | 16.5 µs / 18.8 µs | 37.3 µs / 43.4 µs | 88.1 µs / 110 µs |
| kinds-of 3 levels / branch | - | 16.6 µs / 22.6 µs | 37.4 µs / 39.5 µs | 86.7 µs / 101 µs |
| kinds-of 3 levels (hub) / branch | - | 16.5 µs / 18.7 µs | 36.5 µs / 40.5 µs | 86.5 µs / 105 µs |

### Open + attach + create views, and first lemma query after open

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 |
|---|---|---|---|---|
| open+attach+views /  | 674 µs / 2.6 ms | 771 µs / 1.3 ms | 1.2 ms / 1.8 ms | 2.1 ms / 2.8 ms |
| first lemma query after open /  | 1.6 ms / 3.3 ms | 1.9 ms / 2.1 ms | 2.1 ms / 2.4 ms | 2.9 ms / 3.2 ms |

### Teach write, synchronous=NORMAL (WAL)

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 |
|---|---|---|---|---|
| teach (insert claim+audit, 1 txn) / write | 29.4 µs / 1.1 ms | 28.8 µs / 62.9 µs | 34.5 µs / 82.2 µs | 47.9 µs / 89.9 µs |

### Teach write, synchronous=FULL (WAL)

| case / variant | A p50 / p99 | B p50 / p99 | C p50 / p99 | D p50 / p99 |
|---|---|---|---|---|
| teach (insert claim+audit, 1 txn) / write | 49.5 µs / 3.4 ms | 49.0 µs / 90.5 µs | 54.2 µs / 93.3 µs | 68.3 µs / 204 µs |

### Memory / META

all_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
all_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 17.5
all_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
all_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 25.8
all_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.2
all_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 36.5
all_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
all_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 50.4
cold_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
cold_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
cold_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.3
cold_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
edge_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.8
edge_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | edge_build_ms | 176.7 | edge_rows | 212401
edge_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 23.8
edge_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
edge_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | edge_build_ms | 164.2 | edge_rows | 213002
edge_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 27.8
edge_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.3
edge_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | edge_build_ms | 168.3 | edge_rows | 213002
edge_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 37.4
edge_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
edge_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | edge_build_ms | 170.1 | edge_rows | 213002
edge_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 52.3
open_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 15.4
open_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 15.9
open_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 16.4
open_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 18.0
plans_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
plans_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
plans_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.3
plans_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
prepare_A.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.8
prepare_B.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
prepare_C.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.2
prepare_D.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 39.0
settings.tsv | D | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 101.3
settings.tsv | D | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 120.1
settings.tsv | D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 107.2
settings.tsv | D | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 107.2
settings.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.8
settings.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 39.0
settings.tsv | D | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8
settings.tsv | D | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | peak_rss_mb | 39.0
settings.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
settings.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 16.5
settings.tsv | A | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
settings.tsv | A | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 24.0
settings.tsv | A | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.8
settings.tsv | A | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 93.8
settings.tsv | A | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
settings.tsv | A | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 98.1
settings.tsv | A | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
settings.tsv | A | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 98.1
settings.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 8.9
settings.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 16.5
settings.tsv | A | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | rss_after_open_mb | 8.8
settings.tsv | A | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | peak_rss_mb | 16.6
settings2.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
settings2.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 19.0
settings2.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.2
settings2.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | peak_rss_mb | 24.4
settings2.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.0
settings2.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 18.9
settings2.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.2
settings2.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 24.4
settings2.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.8
settings2.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 38.9
settings2.tsv | B | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.0
settings2.tsv | B | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 99.8
settings2.tsv | C | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.3
settings2.tsv | C | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 102.9
settings2.tsv | D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | rss_after_open_mb | 9.8
settings2.tsv | D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | peak_rss_mb | 107.0
write_A_full.tsv | A | cache=def,mmap=0M,wal=1,sync=FULL,imm=0 | rss_after_open_mb | 8.9
write_A_normal.tsv | A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 8.9
write_B_full.tsv | B | cache=def,mmap=0M,wal=1,sync=FULL,imm=0 | rss_after_open_mb | 9.0
write_B_normal.tsv | B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.0
write_C_full.tsv | C | cache=def,mmap=0M,wal=1,sync=FULL,imm=0 | rss_after_open_mb | 9.2
write_C_normal.tsv | C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.3
write_D_full.tsv | D | cache=def,mmap=0M,wal=1,sync=FULL,imm=0 | rss_after_open_mb | 9.8
write_D_normal.tsv | D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | rss_after_open_mb | 9.8

### Settings sweep (quick suite)

| layout | setting | fts gloss top-20 | isa<=6 | isa<=6 | kinds-of 1 level | lemma->senses | neighbourhood |
|---|---|---|---|---|---|---|---|
| D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | 197 µs / 1.8 ms | 46.2 µs / 74.0 µs | - | 79.3 µs / 103 µs | 154 µs / 190 µs | 318 µs / 442 µs |
| D | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | 194 µs / 1.8 ms | 45.6 µs / 72.0 µs | - | 78.6 µs / 99.8 µs | 154 µs / 190 µs | 304 µs / 381 µs |
| D | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | 196 µs / 1.8 ms | 46.1 µs / 74.9 µs | - | 79.0 µs / 102 µs | 155 µs / 196 µs | 306 µs / 369 µs |
| D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | 189 µs / 1.8 ms | 40.7 µs / 62.8 µs | - | 74.8 µs / 96.2 µs | 150 µs / 188 µs | 292 µs / 346 µs |
| D | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | 190 µs / 1.7 ms | 39.7 µs / 60.0 µs | - | 75.8 µs / 96.1 µs | 152 µs / 194 µs | 295 µs / 354 µs |
| D | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | 170 µs / 1.8 ms | 22.9 µs / 43.7 µs | - | 57.2 µs / 76.9 µs | 134 µs / 173 µs | 253 µs / 312 µs |
| D | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | 195 µs / 1.8 ms | 47.6 µs / 74.8 µs | - | 81.0 µs / 102 µs | 160 µs / 223 µs | 323 µs / 392 µs |
| A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | 44.9 µs / 1.6 ms | - | 11.8 µs / 26.1 µs | 9.1 µs / 130 µs | 18.1 µs / 316 µs | 35.9 µs / 167 µs |
| A | cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 | 40.9 µs / 1.5 ms | - | 9.7 µs / 21.7 µs | 8.5 µs / 133 µs | 18.5 µs / 268 µs | 37.6 µs / 243 µs |
| A | cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 | 40.4 µs / 1.5 ms | - | 8.7 µs / 23.5 µs | 8.3 µs / 133 µs | 18.8 µs / 267 µs | 33.4 µs / 228 µs |
| A | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 | 39.2 µs / 1.5 ms | - | 7.8 µs / 15.2 µs | 7.8 µs / 17.6 µs | 15.8 µs / 332 µs | 29.8 µs / 152 µs |
| A | cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 | 39.4 µs / 1.5 ms | - | 7.8 µs / 15.1 µs | 7.8 µs / 14.4 µs | 15.5 µs / 326 µs | 29.4 µs / 160 µs |
| A | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | 45.4 µs / 1.6 ms | - | 11.7 µs / 24.6 µs | 9.1 µs / 124 µs | 19.0 µs / 264 µs | 37.7 µs / 242 µs |
| A | cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 | 47.2 µs / 1.6 ms | - | 14.0 µs / 30.1 µs | 11.2 µs / 126 µs | 20.2 µs / 331 µs | 41.5 µs / 157 µs |
| B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | 66.1 µs / 1.6 ms | 16.1 µs / 137 µs | - | 18.2 µs / 141 µs | 31.8 µs / 267 µs | 66.3 µs / 326 µs |
| C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 | 107 µs / 1.6 ms | 26.0 µs / 149 µs | - | 36.7 µs / 60.7 µs | 63.5 µs / 288 µs | 135 µs / 394 µs |
| B | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | 53.5 µs / 1.6 ms | 12.8 µs / 25.2 µs | - | 15.0 µs / 27.3 µs | 26.2 µs / 37.5 µs | 55.3 µs / 76.3 µs |
| C | cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 | 93.8 µs / 1.6 ms | 16.8 µs / 36.4 µs | - | 27.8 µs / 42.0 µs | 50.5 µs / 65.6 µs | 106 µs / 142 µs |
| B | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | 50.6 µs / 1.5 ms | 9.2 µs / 17.0 µs | - | 13.6 µs / 21.7 µs | 24.3 µs / 29.6 µs | 48.2 µs / 66.7 µs |
| C | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | 88.2 µs / 1.6 ms | 12.8 µs / 23.9 µs | - | 26.0 µs / 36.2 µs | 48.8 µs / 62.0 µs | 94.3 µs / 116 µs |
| D | cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 | 169 µs / 1.7 ms | 20.9 µs / 39.5 µs | - | 55.6 µs / 71.1 µs | 130 µs / 155 µs | 232 µs / 290 µs |

RSS D cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 39.0
RSS D cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 101.3
RSS D cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 120.1
RSS D cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 107.2
RSS D cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 107.2
RSS D cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 39.0
RSS D cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 39.0
RSS A cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 16.5
RSS A cache=8192,mmap=0M,wal=1,sync=NORMAL,imm=0 24.0
RSS A cache=65536,mmap=0M,wal=1,sync=NORMAL,imm=0 93.8
RSS A cache=def,mmap=256M,wal=1,sync=NORMAL,imm=0 98.1
RSS A cache=65536,mmap=256M,wal=1,sync=NORMAL,imm=0 98.1
RSS A cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 16.5
RSS A cache=def,mmap=0M,wal=0,sync=NORMAL,imm=0 16.6
RSS B cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 19.0
RSS C cache=def,mmap=0M,wal=1,sync=NORMAL,imm=0 24.4
RSS B cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 18.9
RSS C cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 24.4
RSS D cache=def,mmap=0M,wal=1,sync=NORMAL,imm=1 38.9
RSS B cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 99.8
RSS C cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 102.9
RSS D cache=def,mmap=256M,wal=1,sync=NORMAL,imm=1 107.0
