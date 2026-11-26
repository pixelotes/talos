#!/bin/sh
# Size of each binary and peak resident memory of one GET (macOS and Linux).
set -eu
cd "$(dirname "$0")"
URL="${1:-https://es.wikipedia.org/api/rest_v1/page/summary/Valencia}"
cargo build --release --quiet --bin baseline --bin with-curl
cargo build --release --quiet --features ureq --bin with-ureq
peak() {
    if [ "$(uname)" = Darwin ]; then
        /usr/bin/time -l "$@" 2>&1 >/dev/null | awk '/maximum resident/ {printf "%.1f MB", $1/1048576}'
    else
        /usr/bin/time -v "$@" 2>&1 >/dev/null | awk '/Maximum resident/ {printf "%.1f MB", $6/1024}'
    fi
}
for bin in baseline with-curl with-ureq; do
    size=$(wc -c < "target/release/$bin" | tr -d ' ')
    printf '%-10s %8s bytes  peak %s  (%s)\n' "$bin" "$size" "$(peak "target/release/$bin" "$URL")" "$("target/release/$bin" "$URL")"
done
