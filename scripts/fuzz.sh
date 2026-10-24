#!/usr/bin/env bash
# Runs the fuzz targets under fuzz/ (Q-3; ADR-0008, ADR-0015): the mapfile frontend, the style
# document readers, and the render path, each for a number of seconds, on the nightly toolchain
# cargo-fuzz needs. Both are pinned here; rustup and cargo install them when missing. A crash,
# a panic, a hang longer than 10 s, or more than 2 GiB of memory fails the run, and the input
# lands in fuzz/artifacts/. The weekly workflow fuzz.yml runs it; it is too slow for a push.
#
# Usage: bash scripts/fuzz.sh [seconds per target, default 300]
# Verifies: Q-3
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NIGHTLY="nightly-2026-10-20"
CARGO_FUZZ_VERSION="0.13.2"
SECONDS_PER_TARGET="${1:-300}"

cd "$ROOT"
rustup toolchain install "$NIGHTLY" --profile minimal
if ! cargo fuzz --version 2> /dev/null | grep -qF " $CARGO_FUZZ_VERSION"; then
  cargo install --locked cargo-fuzz --version "$CARGO_FUZZ_VERSION"
fi

for target in mapfile style_document render; do
  printf '\n--> fuzz %s for %s s\n' "$target" "$SECONDS_PER_TARGET"
  mkdir -p "fuzz/corpus/$target"
  cargo "+$NIGHTLY" fuzz run "$target" "fuzz/corpus/$target" "fuzz/seeds/$target" -- \
    -max_total_time="$SECONDS_PER_TARGET" -timeout=10 -rss_limit_mb=2048
done
