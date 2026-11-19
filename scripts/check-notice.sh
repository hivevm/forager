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
# subdirectory or not — ports PROJ's formulas (ADR-0037) and must hold
# PROJ's notice: the first words of its terms, the permission sentence, and the disclaimer
# (scripts/lib/notice.sh). THIRD-PARTY-NOTICES.md must then hold the same first words, as the MIT
# licence asks of every copy, binaries included. Each source of PROJ such a file's header names
# must stand in the table of scripts/lib/notice.sh, which holds the copyright lines each source
# holds at PROJ 9.4.0, and the header must hold each of those lines.
# A Rust file under crates/symbolize that names a JTS source — a Java file, such as
# DouglasPeuckerLineSimplifier.java — follows JTS (ADR-draft-geometry-through-geo-and-own-circle)
# and must hold JTS's notice: the copyright line of its sources, and the third condition and the
# disclaimer of the Eclipse Distribution License 1.0 (scripts/lib/notice.sh). THIRD-PARTY-NOTICES.md
# must then hold the same lines, as the licence's second condition asks of a binary. A file that
# names a JTS source of another copyright line, by the table JTS_SOURCE_COPYRIGHTS of
# scripts/lib/notice.sh, must hold that line too, and so must THIRD-PARTY-NOTICES.md.
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
jts=0
jts_failures=0
jts_lines=()
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
  # The sources its header names, each with the copyright lines it holds at PROJ 9.4.0; the
  # header's comment lines joined, so that a line wrapped in it still reads.
  header="$(awk '!/^\/\// { exit } { sub(/^\/\/ ?/, ""); printf "%s ", $0 }' "$file")"
  sources="$(grep -oE '\bsrc/([a-z0-9_]+/)*[A-Za-z0-9_]+\.cpp\b' <<< "$header" | sort -u)"
  while IFS= read -r source; do
    [[ -n "$source" ]] || continue
    if ! lines="$(proj_source_copyrights "$source")"; then
      echo "ERROR: ${file#"$ROOT"/} names $source, whose copyright lines the table of" \
        "scripts/lib/notice.sh does not hold" >&2
      proj_failures=$((proj_failures + 1))
      continue
    fi
    while IFS= read -r line; do
      [[ -n "$line" ]] || continue
      if ! grep -qF "$line" <<< "$header"; then
        echo "ERROR: ${file#"$ROOT"/} follows $source and lacks its line '$line'" >&2
        proj_failures=$((proj_failures + 1))
      fi
    done <<< "$lines"
  done <<< "$sources"
done < <(find "$ROOT/crates/proj" -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)

# The ports of JTS's steps, under crates/symbolize.
while IFS= read -r -d '' file; do
  grep -qE '\b[A-Z][A-Za-z0-9]*\.java\b' "$file" || continue
  jts=$((jts + 1))
  if ! holds_jts "$file"; then
    echo "ERROR: ${file#"$ROOT"/} follows JTS's source and does not hold JTS's notice" >&2
    jts_failures=$((jts_failures + 1))
  fi
  for entry in "${JTS_SOURCE_COPYRIGHTS[@]}"; do
    grep -qF "${entry%%|*}" "$file" || continue
    jts_lines+=("${entry#*|}")
    if ! grep -qF "${entry#*|}" "$file"; then
      echo "ERROR: ${file#"$ROOT"/} follows ${entry%%|*} and lacks its line '${entry#*|}'" >&2
      jts_failures=$((jts_failures + 1))
    fi
  done
done < <(find "$ROOT/crates/symbolize" -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)

# A binary carries the notices in THIRD-PARTY-NOTICES.md, pnpoly's condition 2.
notices="$ROOT/THIRD-PARTY-NOTICES.md"
if ((proj)) && { [[ ! -f "$notices" ]] || ! grep -qF "$PROJ_TERMS" "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce PROJ's notice" >&2
  proj_failures=$((proj_failures + 1))
fi
if ((jts)) && { [[ ! -f "$notices" ]] || ! holds_jts "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce JTS's notice" >&2
  jts_failures=$((jts_failures + 1))
fi
for line in "${jts_lines[@]}"; do
  if [[ ! -f "$notices" ]] || ! grep -qF "$line" "$notices"; then
    echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce JTS's line '$line'" >&2
    jts_failures=$((jts_failures + 1))
  fi
done
if ((pnpoly)) && { [[ ! -f "$notices" ]] || ! holds_third_party_notices "$notices"; }; then
  echo "ERROR: THIRD-PARTY-NOTICES.md does not reproduce W. Randolph Franklin's and MapServer's" \
    "notices" >&2
  pnpoly_failures=$((pnpoly_failures + 1))
fi

if ((failures || pnpoly_failures || proj_failures || jts_failures)); then
  echo "Notice check FAILED ($failures of $checked files that follow MapServer's source;" \
    "$pnpoly_failures problems with the notice of pnpoly, which $pnpoly files follow;" \
    "$proj_failures with PROJ's, which $proj files follow;" \
    "$jts_failures with JTS's, which $jts files follow)." >&2
  exit 1
fi
echo "Notice check passed ($checked files that follow MapServer's source, $pnpoly pnpoly's," \
  "$proj PROJ's, $jts JTS's)."
exit 0
