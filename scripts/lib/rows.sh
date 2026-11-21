# shellcheck shell=bash
# Forager's own: the strict reader of a fixture table, for the self-tests of Forager's checks and
# for the rows of its own that scripts/test-check-docs.sh keeps beside the template's table. A
# reader that skips every line its pattern does not match skips a row of a wrong form without a
# word: Forager's rows that began with a check's number were skipped so when the template changed
# the form, and two of its rules had no test until a review found it.
#
# Sourced after scripts/lib/selftest.sh, whose counter 'failed' a refused line raises.

# table_rows <pattern> — the lines on stdin that match the pattern, one a line. A blank line and a
# comment are skipped. Any other line fails and is named. Run it in the shell of the self-test,
# its output redirected to a file, so that a line it fails on counts.
table_rows() {
  local line
  while IFS= read -r line; do
    if [[ "$line" =~ $1 ]]; then
      printf '%s\n' "$line"
    elif [[ ! "$line" =~ ^([[:space:]]*$|#) ]]; then
      echo "FAIL: a line of the table is no row the reader reads: $line" >&2
      failed=$((failed + 1))
    fi
  done
}

# rows_said <pattern> <line> — what table_rows prints of the line, on stdout and stderr, then a
# line 'failed=<n>', the lines it failed on, counted apart from the self-test's own count.
rows_said() {
  local failed=0
  table_rows "$1" <<< "$2" 2>&1
  echo "failed=$failed"
}

# rows_unread <pattern> <line> — table_rows fails on the line, once, names it, and reads no row of
# it.
# shellcheck disable=SC2317,SC2329 # run by check
rows_unread() {
  [[ "$(rows_said "$1" "$2")" == "FAIL: "*"$2"$'\n'failed=1 ]]
}

# rows_skipped <pattern> <line> — table_rows reads no row of the line and fails on nothing.
# shellcheck disable=SC2317,SC2329 # run by check
rows_skipped() { [[ "$(rows_said "$1" "$2")" == failed=0 ]]; }

# rows_read <pattern> <line> — table_rows reads the line as a row and fails on nothing.
# shellcheck disable=SC2317,SC2329 # run by check
rows_read() { [[ "$(rows_said "$1" "$2")" == "$2"$'\n'failed=0 ]]; }
