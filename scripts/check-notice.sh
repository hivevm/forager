#!/usr/bin/env bash
#
# Check: a Rust or C file whose logic follows MapServer's C code keeps MapServer's copyright and
# permission notice, as the specification's licence constraint requires, and one that follows
# W. Randolph Franklin's pnpoly keeps his; the C file is the reader of the label cache, which
# follows src/apps/map2img.c (ADR-0029).
#
# A Rust or C file under crates/ or fuzz/ that names a MapServer source file — src/map….c, .h,
# .l, or .y — follows that code, and must hold the notice of those files: the copyright line of
# the Regents of the University of Minnesota, the permission sentence, and the disclaimer to its
# last words.
# A file that names msPointInPolygon or pnpoly, a mere mention included, follows W. Randolph
# Franklin's pnpoly, under a notice of its own, and must hold it: his copyright line, his first
# and his third condition, which no other notice holds, and the disclaimer. THIRD-PARTY-NOTICES.md,
# which a binary carries as his second condition asks, must then hold the same lines, and
# MapServer's copyright line.
# Beyond this check's reach are a file that names only a MapServer function, one that names a
# source without `src/`, one that names a C++ source (`src/map….cpp`), whose holders differ by
# file, and a port of pnpoly that names neither word.
#
# Pure bash + grep. Usage:
#     scripts/check-notice.sh [repository root]
# The root defaults to this repository; scripts/test-check-notice.sh passes fixtures.
# Exit code 0 when every such file holds the notice, 1 otherwise, 2 on a root that is no directory.

set -uo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Notice check: '${1:-}' is not a directory." >&2; exit 2; }

COPYRIGHT='Copyright (c) 1996-2005 Regents of the University of Minnesota.'
PERMISSION='Permission is hereby granted, free of charge, to any person obtaining a copy of this software'
DISCLAIMER='OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.'
# W. Randolph Franklin's pnpoly, which MapServer's msPointInPolygon is, under a notice of its own:
# its copyright line, its first condition, and its third, which the other notices do not hold.
PNPOLY_COPYRIGHT='Copyright (c) 1970-2003, Wm. Randolph Franklin'
PNPOLY_CONDITION='Redistributions of source code must retain the above copyright notice'
PNPOLY_ENDORSE='The name of W. Randolph Franklin may not be used to endorse or promote'

# holds_pnpoly <file> — whether the file holds Franklin's notice.
holds_pnpoly() {
  grep -qF "$PNPOLY_COPYRIGHT" "$1" && grep -qF "$PNPOLY_CONDITION" "$1" \
    && grep -qF "$PNPOLY_ENDORSE" "$1" && grep -qF "$DISCLAIMER" "$1"
}

failures=0
checked=0
pnpoly=0
pnpoly_failures=0
while IFS= read -r -d '' file; do
  if grep -qiE '\bmsPointInPolygon\b|\bpnpoly\b' "$file"; then
    pnpoly=$((pnpoly + 1))
    if ! holds_pnpoly "$file"; then
      echo "ERROR: ${file#"$ROOT"/} follows pnpoly and does not hold W. Randolph Franklin's" \
        "notice" >&2
      pnpoly_failures=$((pnpoly_failures + 1))
    fi
  fi
  grep -qE '\bsrc/map[a-z_]*\.(c|h|l|y)\b' "$file" || continue
  checked=$((checked + 1))
  if ! grep -qF "$COPYRIGHT" "$file" || ! grep -qF "$PERMISSION" "$file" \
     || ! grep -qF "$DISCLAIMER" "$file"; then
    echo "ERROR: ${file#"$ROOT"/} follows MapServer's source and does not hold its notice" >&2
    failures=$((failures + 1))
  fi
done < <(find "$ROOT/crates" "$ROOT/fuzz" \( -name '*.rs' -o -name '*.c' \) -not -path '*/target/*' \
  -print0 2>/dev/null)

# A binary carries the notices in THIRD-PARTY-NOTICES.md, pnpoly's condition 2.
notices="$ROOT/THIRD-PARTY-NOTICES.md"
if ((pnpoly)) && { [[ ! -f "$notices" ]] || ! holds_pnpoly "$notices" \
  || ! grep -qF "$COPYRIGHT" "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce W. Randolph Franklin's and MapServer's" \
    "notices" >&2
  pnpoly_failures=$((pnpoly_failures + 1))
fi

if ((failures || pnpoly_failures)); then
  echo "Notice check FAILED ($failures of $checked files that follow MapServer's source;" \
    "$pnpoly_failures problems with the notice of pnpoly, which $pnpoly files follow)." >&2
  exit 1
fi
echo "Notice check passed ($checked files that follow MapServer's source, $pnpoly pnpoly's)."
exit 0
