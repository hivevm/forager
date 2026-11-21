#!/usr/bin/env bash
#
# Self-test of table_rows, the strict reader of a fixture table in scripts/lib/rows.sh, on the
# patterns of the self-tests that read through it: a row whose first field is pass or fail, and a
# row whose first field is a rule key of check-docs.sh or 'all'.
#
# Each case feeds the reader one line and asserts what it prints as a row, what it says on stderr,
# and how many lines it failed on: a row is printed and fails nothing; a blank line, a line of
# spaces, and a comment are neither printed nor failed; any other line is not printed, fails once,
# and is named.
#
# Inputs: scripts/lib/
#
# Usage:
#     scripts/test-rows.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/rows.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/rows.sh"

CASE_RE='^(pass|fail) \| '
RULE_RE='^(docs/[a-z0-9-]+|all) \| '

# read_line <pattern> <line> — feed the line to table_rows: its rows go to $work/rows, what it says
# to $work/said, and the count of lines it failed on is printed, apart from the self-test's own.
read_line() {
  local failed=0
  table_rows "$1" <<< "$2" > "$work/rows" 2> "$work/said"
  echo "$failed"
}

# unread <pattern> <line> — the line fails once, is named, and no row of it is read.
# shellcheck disable=SC2317,SC2329 # run by check
unread() {
  [[ "$(read_line "$1" "$2")" == 1 && ! -s "$work/rows" && "$(< "$work/said")" == *"$2"* ]]
}
# skipped <pattern> <line> — no row of the line is read, and it fails nothing and says nothing.
# shellcheck disable=SC2317,SC2329 # run by check
skipped() { [[ "$(read_line "$1" "$2")" == 0 && ! -s "$work/rows" && ! -s "$work/said" ]]; }
# read_row <pattern> <line> — the line is read as a row, and it fails nothing.
# shellcheck disable=SC2317,SC2329 # run by check
read_row() {
  [[ "$(read_line "$1" "$2")" == 0 && "$(< "$work/rows")" == "$2" && ! -s "$work/said" ]]
}

check "a row of pass is read" read_row "$CASE_RE" "pass | - | a case | -"
check "a row of fail is read" read_row "$CASE_RE" "fail | a/b:c | a case | -"
check "a row whose first field is a check's number fails, named" \
  unread "$CASE_RE" "9 | fail | devcontainer/gh-login:host | the host's gh config | -"
check "a row whose first field is no pass or fail fails, named" \
  unread "$CASE_RE" "passes | - | a case | -"
check "a row whose first field is a rule key fails where pass or fail is read, named" \
  unread "$CASE_RE" "docs/inactive-adr-refs | pass | - | a case | -"
check "a row of pass and no field separator fails, named" unread "$CASE_RE" "pass - a case -"
check "a row of a rule key is read" read_row "$RULE_RE" "docs/links | pass | - | a case | -"
check "a row of all is read" read_row "$RULE_RE" "all | pass | - | a case | -"
check "a row whose first field is a check's number fails where a rule key is read, named" \
  unread "$RULE_RE" "17 | pass | - | a case | -"
check "a row whose first field is no rule key fails, named" \
  unread "$RULE_RE" "docs | pass | - | a case | -"
check "a row of a rule key and no field separator fails, named" \
  unread "$RULE_RE" "docs/links pass - a case -"
check "a comment is no row and fails on nothing" skipped "$CASE_RE" "# pass | - | a case | -"
check "a comment of a rule key is no row and fails on nothing" \
  skipped "$RULE_RE" "# docs/links | pass | - | a case | -"
check "a blank line is no row and fails on nothing" skipped "$CASE_RE" ""
check "a line of spaces is no row and fails on nothing" skipped "$CASE_RE" "   "

# Several lines: each row in its order, and each line it fails on counted.
check "rows are read in their order, and each unread line counts" \
  test "$(read_line "$CASE_RE" $'pass | 1\n# c\nbad\n\nfail | 2\nworse'):$(< "$work/rows")" \
  == $'2:pass | 1\nfail | 2'

summary "Table reader self-test"
