#!/usr/bin/env bash
#
# Self-test of scripts/check-fidelity.sh.
#
# Each case writes the harness's counts and a record as fixtures, runs the check on them, and
# asserts the exit code and a line of its output.
#
# Usage:
#     scripts/test-check-fidelity.sh
# Exit code 0 when every case passes, 1 otherwise.
#
# Verifies: Q-4

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/scripts/check-fidelity.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# fixture <text> — a file holding the text, printed as its path.
fixture() {
  local file
  file="$(mktemp "$work/fixture.XXXXXX")"
  printf '%s\n' "$1" > "$file"
  echo "$file"
}

# counts <in-scope> <passing> [blank] [blank-passing] — the harness's counts, as a file.
counts() {
  fixture "$(printf 'in-scope %s\npassing %s\nblank %s\nblank-passing %s\n' "$1" "$2" "${3:-0}" "${4:-0}")"
}

# expect <name> <code> <needle> <args...> — run the check; it must exit with the code and its
# output contain the needle.
expect() {
  local name="$1" code="$2" needle="$3"
  shift 3
  local out rc
  out="$(bash "$CHECK" "$@" 2>&1)"; rc=$?
  if ((rc == code)) && [[ "$out" == *"$needle"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $code with \"$needle\", exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

RECORD="$(fixture '# a comment
v0.1.0 30/50
v0.2.0 40/50  # the last release
')"

expect "a share below the last release fails and names both shares" 1 \
  "39/50 (78.0%) is below 40/50 (80.0%) of v0.2.0" "$(counts 50 39)" "$RECORD"
expect "the same share passes" 0 "40/50 (80.0%) holds 40/50 (80.0%) of v0.2.0" \
  "$(counts 50 40)" "$RECORD"
expect "a higher share passes" 0 "holds 40/50 (80.0%) of v0.2.0" "$(counts 50 45)" "$RECORD"
expect "cases moved into scope fail when the share falls although more pass" 1 \
  "41/60 (68.3%) is below 40/50 (80.0%) of v0.2.0" "$(counts 60 41)" "$RECORD"
expect "blank cases are left out of the share and reported apart" 1 \
  "38/48 (79.2%) is below 40/50" "$(counts 50 40 2 2)" "$RECORD"
expect "the report names the blank cases apart" 1 \
  "apart from the share, 2 of 2 cases with a blank reference pass" "$(counts 50 40 2 2)" "$RECORD"
expect "no record yet passes and says so" 0 "no release is recorded yet" \
  "$(counts 50 10)" "$(fixture '# no release yet
')"
expect "the last release is the highest version, not the last line" 1 "of v0.10.0" \
  "$(counts 50 44)" "$(fixture 'v0.10.0 45/50
v0.9.0 40/50
')"
expect "a release recorded twice is an error" 2 "v0.1.0 is recorded twice" \
  "$(counts 50 40)" "$(fixture 'v0.1.0 40/50
v0.1.0 41/50
')"
expect "a record line of another form is an error naming its line" 2 "line 2" \
  "$(counts 50 40)" "$(fixture 'v0.1.0 40/50
0.2.0 40/50
')"
expect "a share above one is an error" 2 "51/50 is no share" \
  "$(counts 50 40)" "$(fixture 'v0.1.0 51/50
')"
expect "counts without a line are an error" 2 "does not count blank-passing" \
  "$(fixture 'in-scope 50
passing 40
blank 0
')" "$RECORD"
expect "counts that contradict each other are an error" 2 "contradict" "$(counts 50 51)" "$RECORD"
expect "an undefined share fails once a release is recorded" 1 "the share is undefined" \
  "$(counts 2 2 2 2)" "$RECORD"
expect "an undefined share passes with no release recorded" 0 "no release is recorded yet" \
  "$(counts 2 2 2 2)" "$(fixture '# no release yet')"
expect "an equal share with another number of cases passes" 0 "40/50 (80.0%) holds 48/60 (80.0%)" \
  "$(counts 50 40)" "$(fixture 'v0.1.0 48/60')"
expect "a release recorded twice below the last one is an error" 2 "line 3: v0.1.0 is recorded twice" \
  "$(counts 50 40)" "$(fixture 'v0.2.0 40/50
v0.1.0 30/50
v0.1.0 31/50')"
expect "the check prints the line a release adds" 0 "Record line for a release: v<semver> 40/50" \
  "$(counts 52 42 2 2)" "$RECORD"
expect "missing counts are an error" 2 "did the harness run?" "$work/none" "$RECORD"

if ((failed)); then
  echo "Fidelity check self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Fidelity check self-test passed ($passed cases)."
exit 0
