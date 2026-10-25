#!/usr/bin/env bash
#
# Self-test of scripts/check-notice.sh.
#
# Each case writes a fixture repository with one Rust file, runs the check against it, and asserts
# the exit code and the line that names the problem.
#
# Usage:
#     scripts/test-check-notice.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/scripts/check-notice.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

NOTICE='// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// associated documentation files (the "Software"), to deal in the Software without restriction,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.'
TRUNCATED='// Copyright (c) 1996-2005 Regents of the University of Minnesota.
// Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
// Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED.'

# repo <name> <text> — a fixture whose one file crates/x/src/lib.rs holds the text, printed as its
# path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/crates/x/src"
  printf '%s\n' "$2" > "$dir/crates/x/src/lib.rs"
  echo "$dir"
}

# expect <name> <dir> <code> <needle> — run the check; it must exit with the code and its output
# contain the needle.
expect() {
  local name="$1" dir="$2" code="$3" needle="$4"
  local out rc
  out="$(bash "$CHECK" "$dir" 2>&1)"; rc=$?
  if ((rc == code)) && [[ "$out" == *"$needle"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $code with \"$needle\", exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

expect "a file that cites src/mapfile.c without the notice fails, by name" \
  "$(repo bare '// As loadStyle in src/mapfile.c reads it.')" 1 "crates/x/src/lib.rs follows MapServer"
expect "a file that cites src/maplexer.l without the notice fails" \
  "$(repo lexer '// Token rules follow src/maplexer.l.')" 1 "does not hold its notice"
expect "a file that cites MapServer's source with the notice passes" \
  "$(repo noticed "$NOTICE
// As loadStyle in src/mapfile.c reads it.")" 0 "Notice check passed (1 files"
expect "a notice whose disclaimer stops short fails" \
  "$(repo truncated "$TRUNCATED
// As loadStyle in src/mapfile.c reads it.")" 1 "does not hold its notice"
expect "a file that names only a MapServer function is beyond the check's reach" \
  "$(repo function '// As bindLabel reads it.')" 0 "Notice check passed (0 files"
expect "a file that cites PROJ's source is not MapServer's" \
  "$(repo proj '// As get_init_string in src/init.cpp reads it.')" 0 "Notice check passed (0 files"
expect "a root that is no directory exits 2" "$work/none" 2 "is not a directory"

if ((failed)); then
  echo "Notice check self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Notice check self-test passed ($passed cases)."
exit 0
