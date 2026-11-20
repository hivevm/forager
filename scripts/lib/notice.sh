# shellcheck shell=bash
# The lines of the notices Forager carries, and the tests of a file against them. Sourced by
# scripts/check-notice.sh, which holds the tree's files and THIRD-PARTY-NOTICES.md to them, and
# by scripts/package.sh, which holds each artefact's copy of THIRD-PARTY-NOTICES.md to them.

# shellcheck disable=SC2034 # the sourcing script uses what it needs
COPYRIGHT='Copyright (c) 1996-2005 Regents of the University of Minnesota.'
# The copyright line of src/apps/msencrypt.c, which a file that names it may hold instead.
# shellcheck disable=SC2034
COPYRIGHT_2006='Copyright (c) 1996-2006 Regents of the University of Minnesota.'
# shellcheck disable=SC2034
PERMISSION='Permission is hereby granted, free of charge, to any person obtaining a copy of this software'
DISCLAIMER='OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.'
# W. Randolph Franklin's pnpoly, which MapServer's msPointInPolygon is, under a notice of its own:
# its copyright line, its first condition, and its third, which the other notices do not hold.
PNPOLY_COPYRIGHT='Copyright (c) 1970-2003, Wm. Randolph Franklin'
PNPOLY_CONDITION='Redistributions of source code must retain the above copyright notice'
PNPOLY_ENDORSE='The name of W. Randolph Franklin may not be used to endorse or promote'

# PROJ's notice, from its file COPYING, which a port of its formulas under crates/proj holds
# (GEO-proj-ported-with-libm): its terms' first words, which no other notice holds, and
# the permission sentence and the disclaimer of the MIT licence, which MapServer's share.
PROJ_TERMS='All source, data files and other contents of the PROJ package are available'

# JTS's notice, which a file under crates/symbolize that follows a JTS source holds (GEO-own-circle-for-buffer): the
# copyright line of those sources, and the third condition and the disclaimer of the Eclipse
# Distribution License 1.0, under which Forager takes JTS.
JTS_COPYRIGHT='Copyright (c) 2016 Vivid Solutions.'
JTS_ENDORSE='Neither the name of the Eclipse Foundation, Inc. nor the names of its'
JTS_DISCLAIMER='THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY'
# The JTS sources whose copyright line is another than JTS_COPYRIGHT, as "<source>|<line>", read
# from the sources at JTS's master 722a943c, and at its tag 1.19.0 for the sources of the circle
# of a point's buffer. A port that names such a source holds its line, and THIRD-PARTY-NOTICES.md
# holds it too.
JTS_SOURCE_COPYRIGHTS=(
  'RobustLineIntersector.java|Copyright (c) 2016 Vivid Solutions, and others.'
  'ComponentJumpChecker.java|Copyright (c) 2023 Martin Davis.'
  'OffsetSegmentGenerator.java|Copyright (c) 2016 Martin Davis.'
  'OffsetSegmentString.java|Copyright (c) 2016 Martin Davis.'
)

# The copyright lines each source of PROJ 9.4.0 holds that a port under crates/proj names, as
# "<source>|<line>;<line>", each line without its "Copyright (c) "; nothing after the bar for a
# source that holds none. Read from the sources at the tag 9.4.0; a port names a source the table
# lacks only once a line for it is added here.
PROJ_SOURCE_COPYRIGHTS=(
  'src/adjlon.cpp|'
  'src/msfn.cpp|'
  'src/phi2.cpp|'
  'src/tsfn.cpp|'
  'src/ellps.cpp|'
  'src/units.cpp|'
  'src/ell_set.cpp|'
  'src/datums.cpp|2000, Frank Warmerdam'
  'src/datum_set.cpp|2000, Frank Warmerdam'
  'src/init.cpp|1995, Gerald Evenden;2002, Frank Warmerdam'
  'src/fwd.cpp|2000, Frank Warmerdam;2018, Thomas Knudsen / SDFE'
  'src/inv.cpp|2000, Frank Warmerdam;2018, Thomas Knudsen / SDFE'
  'src/4D_api.cpp|2016, 2017 Thomas Knudsen/SDFE'
  'src/conversions/cart.cpp|2016, Thomas Knudsen / SDFE'
  'src/transformations/helmert.cpp|2016, Thomas Knudsen / SDFE'
  'src/projections/lcc.cpp|'
  'src/projections/merc.cpp|'
  'src/projections/ortho.cpp|'
  'src/projections/tmerc.cpp|'
)

# proj_source_copyrights <source> — prints the copyright lines of a PROJ source, one per line,
# each with its "Copyright (c) "; exit code 1 for a source the table does not hold.
proj_source_copyrights() {
  local entry lines line
  for entry in "${PROJ_SOURCE_COPYRIGHTS[@]}"; do
    [[ "${entry%%|*}" == "$1" ]] || continue
    lines="${entry#*|}"
    [[ -n "$lines" ]] || return 0
    IFS=';' read -r -a parts <<< "$lines"
    for line in "${parts[@]}"; do
      printf 'Copyright (c) %s\n' "$line"
    done
    return 0
  done
  return 1
}

# holds_proj <file> — whether the file holds PROJ's notice.
holds_proj() {
  grep -qF "$PROJ_TERMS" "$1" && grep -qF "$PERMISSION" "$1" && grep -qF "$DISCLAIMER" "$1"
}

# holds_jts <file> — whether the file holds JTS's notice.
holds_jts() {
  grep -qF "$JTS_COPYRIGHT" "$1" && grep -qF "$JTS_ENDORSE" "$1" && grep -qF "$JTS_DISCLAIMER" "$1"
}

# holds_pnpoly <file> — whether the file holds Franklin's notice.
holds_pnpoly() {
  grep -qF "$PNPOLY_COPYRIGHT" "$1" && grep -qF "$PNPOLY_CONDITION" "$1" \
    && grep -qF "$PNPOLY_ENDORSE" "$1" && grep -qF "$DISCLAIMER" "$1"
}

# holds_third_party_notices <file> — whether the file holds what THIRD-PARTY-NOTICES.md must:
# Franklin's notice, as his second condition asks of a binary, and MapServer's copyright line.
holds_third_party_notices() {
  holds_pnpoly "$1" && grep -qF "$COPYRIGHT" "$1"
}
