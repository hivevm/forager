#!/usr/bin/env bash
#
# Check: every guard against a third-party crate is held to the version Cargo.lock pins
# (ADR-0049).
#
# A guard carries the marker '// Guard: <crate> <version>' on a line of its own above it, in a Rust
# file under crates/. This check lists each marker, and fails where:
#
#   1. Cargo.lock holds the crate, but at no version the marker names. A crate held at two
#      versions, as a crate taken directly and another version of it taken through a dependency
#      are, matches where one of them is the marker's;
#   2. Cargo.lock does not hold the crate;
#   3. a line that begins '// Guard:' does not name one crate and one version;
#   4. the tree has no Cargo.lock.
#
# That a guard was derived from the source lines it names, and that every guard carries a marker,
# no check decides; the review does (ADR-0049, Enforcement).
#
# Pure bash + grep and awk. Usage:
#     scripts/check-guards.sh [repository root]
# The root defaults to this repository; scripts/test-check-guards.sh passes fixtures.
# Exit code 0 when every marker names a locked version, 1 otherwise, 2 on a root that is no
# directory.

set -uo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Guard check: '${1:-}' is not a directory." >&2; exit 2; }
LOCK="$ROOT/Cargo.lock"

failures=0
fail() { echo "ERROR: $*" >&2; failures=$((failures + 1)); }

if [[ ! -f "$LOCK" ]]; then
  echo "ERROR: there is no Cargo.lock under $ROOT to hold the guards' versions to" >&2
  echo "Guard check FAILED." >&2
  exit 1
fi

# Each package of the lock as '<name> <version>', one a line.
locked="$(awk '
  /^\[\[package\]\]/ { name = "" }
  /^name = "/ { name = $3; gsub(/"/, "", name) }
  /^version = "/ && name != "" { v = $3; gsub(/"/, "", v); print name, v; name = "" }
' "$LOCK")"

# The markers, as '<file>:<line>:<text>'. Collected first, so that a failing grep fails the check
# instead of listing nothing; grep exits 1 where no file holds a marker.
markers="$(grep -rnE --include='*.rs' --exclude-dir=target '^[[:space:]]*// Guard:' \
  "$ROOT/crates" 2>/dev/null)"
(($? > 1)) && fail "cannot read the Rust files under crates/"

count=0
while IFS= read -r marker; do
  [[ -n "$marker" ]] || continue
  file="${marker%%:*}"
  rest="${marker#*:}"
  line="${rest%%:*}"
  text="${rest#*:}"
  where="${file#"$ROOT"/}:$line"
  if [[ ! "$text" =~ ^[[:space:]]*//\ Guard:\ ([A-Za-z0-9_-]+)\ ([0-9]+\.[0-9]+\.[0-9]+[A-Za-z0-9.+-]*)[[:space:]]*$ ]]; then
    fail "$where holds a guard marker that is not '// Guard: <crate> <version>'"
    continue
  fi
  crate="${BASH_REMATCH[1]}" version="${BASH_REMATCH[2]}"
  count=$((count + 1))
  echo "$where: $crate $version"
  versions="$(awk -v c="$crate" '$1 == c { printf "%s%s", sep, $2; sep = " " }' <<< "$locked")"
  if [[ -z "$versions" ]]; then
    fail "$where names $crate $version, which Cargo.lock does not hold"
  elif ! grep -qxF "$version" <<< "${versions// /$'\n'}"; then
    fail "$where names $crate $version, but Cargo.lock holds $crate $versions; re-derive the" \
      "guard from that version and update its marker (ADR-0049)"
  fi
done <<< "$markers"

if ((failures)); then
  echo "Guard check FAILED ($failures problem(s) among $count marker(s))." >&2
  exit 1
fi
noun=markers
((count == 1)) && noun=marker
echo "Guard check passed ($count $noun)."
