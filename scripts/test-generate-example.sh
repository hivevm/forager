#!/usr/bin/env bash
#
# Self-test of scripts/generate-example.sh, and the check that the browser example's GeoJSON is
# what it writes.
#
# It runs the generator twice, each time into an empty directory of its own, and fails when the
# two runs differ in a byte or in the files they write, or when a file differs from the one of the
# same name under examples/web/, so that the checked-in data can always be made again.
#
# Pure bash + coreutils (cmp) and the generator's awk. Usage:
#     scripts/test-generate-example.sh
# Exit code 0 when both runs match each other and the checked-in files, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GENERATOR="$ROOT/scripts/generate-example.sh"
EXAMPLE="$ROOT/examples/web"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0
fail() { echo "ERROR: $*" >&2; failures=$((failures + 1)); }

for run in first second; do
  bash "$GENERATOR" "$work/$run" || fail "the generator failed in its $run run"
done

first="$(cd "$work/first" && ls)"
second="$(cd "$work/second" && ls)"
[[ -n "$first" ]] || fail "the generator wrote no file"
[[ "$first" == "$second" ]] || fail "the two runs wrote different files: $first against $second"

for name in $first; do
  cmp -s "$work/first/$name" "$work/second/$name" \
    || fail "$name differs between two runs of the generator"
  cmp -s "$work/first/$name" "$EXAMPLE/$name" \
    || fail "examples/web/$name is not what scripts/generate-example.sh writes; run it and commit"
done

if ((failures)); then
  echo "Example generator self-test: $failures problem(s)." >&2
  exit 1
fi
echo "Example generator self-test passed: $(wc -w <<< "$first" | tr -d ' ') files, the same in two runs and in examples/web/."
