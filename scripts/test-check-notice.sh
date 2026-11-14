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

# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

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

# expect_check <name> <dir> <code> <needle> — run the check; it must exit with the code and its
# output contain the needle (expect, scripts/lib/expect.sh).
expect_check() { expect "$1" "$3" "$4" bash "$CHECK" "$2"; }

expect_check "a file that cites src/mapfile.c without the notice fails, by name" \
  "$(repo bare '// As loadStyle in src/mapfile.c reads it.')" 1 "crates/x/src/lib.rs follows MapServer"
expect_check "a file that cites src/maplexer.l without the notice fails" \
  "$(repo lexer '// Token rules follow src/maplexer.l.')" 1 "does not hold its notice"
expect_check "a file that cites MapServer's source with the notice passes" \
  "$(repo noticed "$NOTICE
// As loadStyle in src/mapfile.c reads it.")" 0 "Notice check passed (1 files"
expect_check "a notice whose disclaimer stops short fails" \
  "$(repo truncated "$TRUNCATED
// As loadStyle in src/mapfile.c reads it.")" 1 "does not hold its notice"
expect_check "a file that names only a MapServer function is beyond the check's reach" \
  "$(repo function '// As bindLabel reads it.')" 0 "Notice check passed (0 files"
expect_check "a file that cites PROJ's source is not MapServer's" \
  "$(repo proj '// As get_init_string in src/init.cpp reads it.')" 0 "Notice check passed (0 files"
cdir="$work/cfile"
mkdir -p "$cdir/crates/x/tests"
printf '%s\n' '/* Walks the cache as msDrawLabelCache in src/mapdraw.c does. */' > "$cdir/crates/x/tests/reader.c"
expect_check "a C file that cites MapServer's source without the notice fails" \
  "$cdir" 1 "crates/x/tests/reader.c follows MapServer"
adir="$work/apps"
mkdir -p "$adir/crates/x/tests"
printf '%s\n' '/* Walks the cache as main in src/apps/map2img.c does. */' > "$adir/crates/x/tests/reader.c"
expect_check "a C file that cites only src/apps/map2img.c without the notice fails" \
  "$adir" 1 "crates/x/tests/reader.c follows MapServer"
expect_check "a Rust file that cites only src/apps/map2img.c without the notice fails" \
  "$(repo appsrust '// As main in src/apps/map2img.c renders it.')" 1 "does not hold its notice"
expect_check "a file that cites src/apps/map2img.c with the notice passes" \
  "$(repo appsnoticed "$NOTICE
// As main in src/apps/map2img.c renders it.")" 0 "Notice check passed (1 files"
expect_check "a file that cites only src/apps/coshp.c, Paul Ramsey's, is not held to the Regents' notice" \
  "$(repo appscoshp '// As src/apps/coshp.c sorts it.')" 0 "Notice check passed (0 files"
expect_check "a file that cites src/apps/coshp.c and a MapServer source without the notice fails" \
  "$(repo appscoshpmap '// As src/apps/coshp.c and src/mapshape.c read it.')" 1 "does not hold its notice"
expect_check "a file that cites src/apps/msencrypt.c with its 1996-2006 notice passes" \
  "$(repo appsencrypt "${NOTICE/1996-2005/1996-2006}
// As src/apps/msencrypt.c reads the key.")" 0 "Notice check passed (1 files"
expect_check "a file that cites only src/mapshape.c with the 1996-2006 line fails" \
  "$(repo map2006 "${NOTICE/1996-2005/1996-2006}
// As src/mapshape.c reads it.")" 1 "does not hold its notice"
expect_check "a file that cites a source a thousand times without the notice fails" \
  "$(repo manyrefs "$(for _ in $(seq 1000); do echo '// src/mapshape.c'; done)")" 1 \
  "does not hold its notice"
PNPOLY_LINE='// Copyright (c) 1970-2003, Wm. Randolph Franklin'
PNPOLY_ONE='// 1. Redistributions of source code must retain the above copyright notice, this list of'
PNPOLY_THREE='// 3. The name of W. Randolph Franklin may not be used to endorse or promote products'
PNPOLY_END='// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.'
PNPOLY="$PNPOLY_LINE
$PNPOLY_ONE
$PNPOLY_THREE
$PNPOLY_END"
expect_check "a file that names pnpoly without Franklin's notice fails, by name" \
  "$(repo pnpolyname '// The crossing test of pnpoly.')" 1 "crates/x/src/lib.rs follows pnpoly"
expect_check "Franklin's copyright line alone fails" \
  "$(repo pnpolyline "$PNPOLY_LINE
// As msPointInPolygon tests it.")" 1 "follows pnpoly"
expect_check "Franklin's notice without his third condition fails" \
  "$(repo pnpolynothree "$PNPOLY_LINE
$PNPOLY_ONE
$PNPOLY_END
// As msPointInPolygon tests it.")" 1 "follows pnpoly"
expect_check "a file that ports msPointInPolygon without Franklin's notice fails, by name" \
  "$(repo pnpolybare '// As msPointInPolygon tests it.')" 1 "crates/x/src/lib.rs follows pnpoly"
pdir="$(repo pnpolynotice "$PNPOLY
// As msPointInPolygon tests it.")"
expect_check "Franklin's notice without THIRD-PARTY-NOTICES.md reproducing it fails" \
  "$pdir" 1 "THIRD-PARTY-NOTICES.md does not reproduce"
printf '%s\n' "$PNPOLY_LINE" > "$pdir/THIRD-PARTY-NOTICES.md"
expect_check "THIRD-PARTY-NOTICES.md with only Franklin's copyright line fails" \
  "$pdir" 1 "THIRD-PARTY-NOTICES.md does not reproduce"
printf '%s\n' "$PNPOLY" > "$pdir/THIRD-PARTY-NOTICES.md"
expect_check "THIRD-PARTY-NOTICES.md without MapServer's notice fails" \
  "$pdir" 1 "THIRD-PARTY-NOTICES.md does not reproduce"
printf '%s\n' "$PNPOLY" "$NOTICE" > "$pdir/THIRD-PARTY-NOTICES.md"
expect_check "Franklin's notice, reproduced in THIRD-PARTY-NOTICES.md beside MapServer's, passes" \
  "$pdir" 0 "Notice check passed"
expect_check "a root that is no directory exits 2" "$work/none" 2 "is not a directory"

if ((failed)); then
  echo "Notice check self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Notice check self-test passed ($passed cases)."
exit 0
