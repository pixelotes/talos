#!/bin/sh
# For each server binary: stripped size, RSS at rest, RSS after 1 000 and
# 10 000 keep-alive requests, round-trip p50/p99 (microseconds) of the
# 10 000. Build first:
#   cargo build --release --bin std-hello --bin load
#   cargo build --release --features axum --bin axum-hello
#   cargo build --release --features tiny --bin tiny-hello
# Usage: ./measure.sh [server-binary ...]; a Talos binary is measured with
# `talos serve --port 0` when its name is `talos`.
cd "$(dirname "$0")"
LOAD=./target/release/load
for BIN in "${@:-./target/release/std-hello ./target/release/tiny-hello ./target/release/axum-hello}"; do
  for B in $BIN; do
    OUT=$(mktemp)
    case "$(basename "$B")" in
      talos) "$B" serve --port 0 2>"$OUT" & ;;
      *) "$B" >"$OUT" & ;;
    esac
    PID=$!
    sleep 1
    ADDR=$(grep -Eo '127\.0\.0\.1:[0-9]+' "$OUT" | head -1)
    REST=$(ps -o rss= -p $PID | tr -d ' ')
    $LOAD "$ADDR" 1000 >/dev/null
    AFTER1K=$(ps -o rss= -p $PID | tr -d ' ')
    LAT=$($LOAD "$ADDR" 10000)
    AFTER10K=$(ps -o rss= -p $PID | tr -d ' ')
    SIZE=$(stat -f %z "$B" 2>/dev/null || stat -c %s "$B")
    printf '%s\tsize=%s B\trss_rest=%s KB\trss_1k=%s KB\trss_11k=%s KB\t%s\n' \
      "$(basename "$B")" "$SIZE" "$REST" "$AFTER1K" "$AFTER10K" "$LAT"
    kill $PID
    rm -f "$OUT"
  done
done
