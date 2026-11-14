#!/usr/bin/env bash
#
# Check: a Rust or C file whose logic follows MapServer's C code keeps MapServer's copyright and
# permission notice, as the specification's licence constraint requires, and one that follows
# W. Randolph Franklin's pnpoly keeps his; the C file is the reader of the label cache, which
# follows src/apps/map2img.c (ADR-0033).
#
# A Rust or C file under crates/ or fuzz/ that names a MapServer source file — src/map….c, .h,
# .l, or .y, or a program's src/apps/….c — follows that code, and must hold the notice of those
# files: the copyright line of the Regents of the University of Minnesota, the permission
# sentence, and the disclaimer to its last words. src/apps/map2img.c at rel-8-6-6 carries the
# same notice (l.9-27), as do the other programs under src/apps/ but two: src/apps/msencrypt.c
# names the years 1996-2006, which a file that names it may hold instead, and src/apps/coshp.c
# is Paul Ramsey's (2021), not the Regents', so a file that names only it is not held to their
# notice (the maintainer's decision of 2026-11-08); a port of it would take a notice of its own,
# as pnpoly's.
# A file that names msPointInPolygon or pnpoly, a mere mention included, follows W. Randolph
# Franklin's pnpoly, under a notice of its own, and must hold it: his copyright line, his first
# and his third condition, which no other notice holds, and the disclaimer. THIRD-PARTY-NOTICES.md,
# which a binary carries as his second condition asks, must then hold the same lines, and
# MapServer's copyright line.
# A Rust file under crates/proj that names a source file of PROJ — src/….cpp or .h, in a
# subdirectory or not — ports PROJ's formulas (ADR-draft-reprojection-ported-in-proj) and must hold
# PROJ's notice: the first words of its terms, the permission sentence, and the disclaimer
# (scripts/lib/notice.sh). THIRD-PARTY-NOTICES.md must then hold the same first words, as the MIT
# licence asks of every copy, binaries included.
# Beyond this check's reach are a file that names only a MapServer function, one that names a
# source without `src/`, one that names a C++ source (`src/map….cpp`), whose holders differ by
# file, and a port of pnpoly that names neither word.
#
# Pure bash + grep; the notices' lines are in scripts/lib/notice.sh, which scripts/package.sh
# shares. Usage:
#     scripts/check-notice.sh [repository root]
# The root defaults to this repository; scripts/test-check-notice.sh passes fixtures.
# Exit code 0 when every such file holds the notice, 1 otherwise, 2 on a root that is no directory.

set -uo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Notice check: '${1:-}' is not a directory." >&2; exit 2; }

# shellcheck source-path=SCRIPTDIR source=lib/notice.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/notice.sh"

failures=0
checked=0
pnpoly=0
pnpoly_failures=0
proj=0
proj_failures=0
while IFS= read -r -d '' file; do
  if grep -qiE '\bmsPointInPolygon\b|\bpnpoly\b' "$file"; then
    pnpoly=$((pnpoly + 1))
    if ! holds_pnpoly "$file"; then
      echo "ERROR: ${file#"$ROOT"/} follows pnpoly and does not hold W. Randolph Franklin's" \
        "notice" >&2
      pnpoly_failures=$((pnpoly_failures + 1))
    fi
  fi
  # Captured, not piped into grep -q: under pipefail a long list would fail the pipe by SIGPIPE.
  refs="$(grep -oE '\bsrc/(map[a-z_]*\.(c|h|l|y)|apps/[a-z0-9_]+\.c)\b' "$file" \
    | grep -vx 'src/apps/coshp.c')"
  [[ -n "$refs" ]] || continue
  checked=$((checked + 1))
  copyright="$COPYRIGHT"
  if grep -qx 'src/apps/msencrypt.c' <<< "$refs" && ! grep -qF "$COPYRIGHT" "$file"; then
    copyright="$COPYRIGHT_2006"
  fi
  if ! grep -qF "$copyright" "$file" || ! grep -qF "$PERMISSION" "$file" \
     || ! grep -qF "$DISCLAIMER" "$file"; then
    echo "ERROR: ${file#"$ROOT"/} follows MapServer's source and does not hold its notice" >&2
    failures=$((failures + 1))
  fi
done < <(find "$ROOT/crates" "$ROOT/fuzz" \( -name '*.rs' -o -name '*.c' \) -not -path '*/target/*' \
  -print0 2>/dev/null)

# The ports of PROJ's formulas, under crates/proj.
while IFS= read -r -d '' file; do
  grep -qE '\bsrc/([a-z0-9_]+/)*[A-Za-z0-9_]+\.(cpp|h)\b' "$file" || continue
  proj=$((proj + 1))
  if ! holds_proj "$file"; then
    echo "ERROR: ${file#"$ROOT"/} follows PROJ's source and does not hold PROJ's notice" >&2
    proj_failures=$((proj_failures + 1))
  fi
done < <(find "$ROOT/crates/proj" -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)

# A binary carries the notices in THIRD-PARTY-NOTICES.md, pnpoly's condition 2.
notices="$ROOT/THIRD-PARTY-NOTICES.md"
if ((proj)) && { [[ ! -f "$notices" ]] || ! grep -qF "$PROJ_TERMS" "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce PROJ's notice" >&2
  proj_failures=$((proj_failures + 1))
fi
if ((pnpoly)) && { [[ ! -f "$notices" ]] || ! holds_third_party_notices "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce W. Randolph Franklin's and MapServer's" \
    "notices" >&2
  pnpoly_failures=$((pnpoly_failures + 1))
fi

if ((failures || pnpoly_failures || proj_failures)); then
  echo "Notice check FAILED ($failures of $checked files that follow MapServer's source;" \
    "$pnpoly_failures problems with the notice of pnpoly, which $pnpoly files follow;" \
    "$proj_failures with PROJ's, which $proj files follow)." >&2
  exit 1
fi
echo "Notice check passed ($checked files that follow MapServer's source, $pnpoly pnpoly's," \
  "$proj PROJ's)."
exit 0
