#!/usr/bin/env bash
# The mutation testing of ADR-0008: cargo-mutants changes the code one small edit at a time and
# reports each edit no test notices — a test that runs without deciding. A sensor: it reports
# what survived and fails only when the run itself breaks, not on what it counts. The weekly
# workflow mutants.yml runs it; locally a shard, such as 1/20, keeps it short.
#
# Usage: bash scripts/mutants.sh [shard, e.g. 1/4; default all]
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_MUTANTS_VERSION="27.1.0"
SHARD="${1:-}"

cd "$ROOT" || exit 1
if ! cargo mutants --version 2> /dev/null | grep -qF " $CARGO_MUTANTS_VERSION"; then
  cargo install --locked cargo-mutants --version "$CARGO_MUTANTS_VERSION" || exit 1
fi

args=(--workspace --test-workspace=false --no-shuffle --output target/mutants)
[[ -n "$SHARD" ]] && args+=(--shard "$SHARD")
cargo mutants "${args[@]}"
status=$?

# cargo-mutants: 0 every mutant caught, 2 some missed, 3 some timed out — the findings this
# sensor reports; anything else is a broken run.
report=target/mutants/mutants.out
if [[ -f "$report/missed.txt" ]]; then
  printf '\nMutants no test caught (%s):\n' "$(wc -l < "$report/missed.txt")"
  cat "$report/missed.txt"
fi
case "$status" in
  0 | 2 | 3) exit 0 ;;
  *) echo "mutants: the run failed (exit $status)" >&2; exit 1 ;;
esac
