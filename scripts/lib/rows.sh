# shellcheck shell=bash
# Forager's own: the strict reader of a fixture table, for the self-tests of Forager's checks and
# for the rows of its own that scripts/test-check-docs.sh keeps beside the template's table. A
# reader that skips every line its pattern does not match skips a row of a wrong form without a
# word: Forager's rows that began with a check's number were skipped so when the template changed
# the form, and two of its rules had no test until a review found it.
#
# Sourced after scripts/lib/selftest.sh, whose counter 'failed' a refused line raises. Its
# self-test is scripts/test-rows.sh.

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
