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
