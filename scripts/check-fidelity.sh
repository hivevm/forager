#!/usr/bin/env bash
#
# Check: the share of in-scope msautotest cases that pass does not fall below the share of the
# last release (Q-4).
#
# The conformance harness (crates/cli/tests/msautotest.rs) writes its counts to the file named by
# FORAGER_FIDELITY, one `<name> <number>` line each:
#     in-scope <n>        the cases the list puts in scope
#     passing <n>         those of them that pass
#     blank <n>           those of the in-scope cases whose reference the list marks blank
#     blank-passing <n>   those of the passing cases whose reference the list marks blank
# The share is that of the cases whose reference is not blank: a blank reference passes against
# nearly any rendering of one colour, so the blank cases are reported apart and do not count.
#
# The record, crates/cli/tests/conformance/fidelity.txt, holds one line per release tag
# `v<semver>`, `v<semver> <passing>/<in-scope>`, the cases without a blank reference; `#` begins
# a comment. The last release is the highest version of the record. A release adds its line.
#
# Pure bash + coreutils. scripts/check-rust.sh runs it after the harness. Usage:
#     scripts/check-fidelity.sh <counts> [record]
# The record defaults to this repository's; scripts/test-check-fidelity.sh passes fixtures.
# Exit code 0 when the share holds or no release is recorded yet, 1 when it falls, 2 on a counts
# file or a record that cannot be read.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
COUNTS="${1:-}"
RECORD="${2:-$ROOT/crates/cli/tests/conformance/fidelity.txt}"

# fail <message> — a counts file or a record that cannot be read.
fail() { echo "Fidelity check: $1" >&2; exit 2; }

# percent <passing> <in-scope> — the share, to one decimal.
percent() { awk -v p="$1" -v n="$2" 'BEGIN { printf "%.1f%%", 100 * p / n }'; }

[[ -n "$COUNTS" ]] || fail "usage: check-fidelity.sh <counts> [record]"
[[ -f "$COUNTS" ]] || fail "the counts '$COUNTS' are no file; did the harness run?"
[[ -f "$RECORD" ]] || fail "the record '$RECORD' is no file"

declare -A count=()
while read -r name value rest || [[ -n "${name:-}" ]]; do
  [[ -z "$name" ]] && continue
  [[ "$name" =~ ^(in-scope|passing|blank|blank-passing)$ && "$value" =~ ^[0-9]+$ && -z "$rest" ]] \
    || fail "'$COUNTS': '$name $value $rest' is no count"
  [[ -z "${count[$name]:-}" ]] || fail "'$COUNTS': $name is counted twice"
  count[$name]=$((10#$value))
done < "$COUNTS"
for name in in-scope passing blank blank-passing; do
  [[ -n "${count[$name]:-}" ]] || fail "'$COUNTS' does not count $name"
done
((count[blank] <= count[in-scope] && count[passing] <= count[in-scope] \
  && count[blank-passing] <= count[blank] && count[blank-passing] <= count[passing])) \
  || fail "'$COUNTS': the counts contradict each other"

passing=$((count[passing] - count[blank-passing]))
in_scope=$((count[in-scope] - count[blank]))
((in_scope > 0)) || { echo "Fidelity check FAILED: no case without a blank reference is in scope; the share is undefined." >&2; exit 1; }
echo "Fidelity: $passing of $in_scope cases pass ($(percent "$passing" "$in_scope")); apart from the share, $((count[blank-passing])) of $((count[blank])) cases with a blank reference pass."

last="" last_passing=0 last_in_scope=0 number=0
while IFS= read -r line || [[ -n "$line" ]]; do
  number=$((number + 1))
  line="${line%%#*}"
  read -r tag share rest <<< "$line"
  [[ -z "${tag:-}" ]] && continue
  [[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ \
     && "${share:-}" =~ ^([0-9]+)/([0-9]+)$ && -z "${rest:-}" ]] \
    || fail "'$RECORD' line $number: '$line' is no 'v<semver> <passing>/<in-scope>'"
  p=$((10#${BASH_REMATCH[1]})) n=$((10#${BASH_REMATCH[2]}))
  ((n > 0 && p <= n)) || fail "'$RECORD' line $number: $share is no share"
  if [[ -n "$last" && "$(printf '%s\n%s\n' "${last#v}" "${tag#v}" | sort -V | tail -n 1)" == "${last#v}" ]]; then
    [[ "$tag" != "$last" ]] || fail "'$RECORD' line $number: $tag is recorded twice"
    continue
  fi
  last="$tag" last_passing=$p last_in_scope=$n
done < "$RECORD"

if [[ -z "$last" ]]; then
  echo "Fidelity check passed: no release is recorded yet, so there is no share to hold."
  exit 0
fi
# Integer cross-multiplication: passing/in_scope < last_passing/last_in_scope.
if ((passing * last_in_scope < last_passing * in_scope)); then
  echo "Fidelity check FAILED: $passing/$in_scope ($(percent "$passing" "$in_scope")) is below $last_passing/$last_in_scope ($(percent "$last_passing" "$last_in_scope")) of $last (Q-4)." >&2
  exit 1
fi
echo "Fidelity check passed: $passing/$in_scope ($(percent "$passing" "$in_scope")) holds $last_passing/$last_in_scope ($(percent "$last_passing" "$last_in_scope")) of $last."
exit 0
