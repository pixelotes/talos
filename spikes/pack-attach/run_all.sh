#!/bin/sh
# Full benchmark run. Output: out/*.tsv (RESULT/META/PLAN lines). Takes ~1 hour.
cd "$(dirname "$0")"
B=./target/release/pack-attach
mkdir -p out
$B bench A --suite baseline > out/baseline.tsv
for L in A B C D; do
  $B bench $L --suite all   --runs 3000 > out/all_$L.tsv
  $B bench $L --suite edge  --runs 3000 --edge > out/edge_$L.tsv
  $B bench $L --suite cold  --runs 2000 > out/cold_$L.tsv
  $B bench $L --suite open  > out/open_$L.tsv
  $B bench $L --suite prepare > out/prepare_$L.tsv
  $B bench $L --suite plans > out/plans_$L.tsv
  $B bench $L --suite write --runs 3000 --sync NORMAL > out/write_${L}_normal.tsv
  $B bench $L --suite write --runs 3000 --sync FULL > out/write_${L}_full.tsv
done
# settings sweep (layout D and A), quick suite
for L in D A; do
  for S in "" "--cache 8192" "--cache 65536" "--mmap 256" "--cache 65536 --mmap 256" "--immutable" "--wal 0"; do
    $B bench $L --suite quick --runs 3000 $S
  done
done > out/settings.tsv
echo done > out/DONE
