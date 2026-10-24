#!/usr/bin/env bash
#
# Check: a Rust file whose logic follows MapServer's C code keeps MapServer's copyright and
# permission notice, as the specification's licence constraint requires.
#
# A file under crates/ or fuzz/ that names a MapServer source file — src/map….c, .h, .l, or .y —
# follows that code, and must hold the notice: the copyright line of the Regents of the University
# of Minnesota and the permission sentence of MapServer's licence. A file that names only a
# MapServer function, without its source file, is beyond this check's reach.
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

failures=0
checked=0
while IFS= read -r -d '' file; do
  grep -qE '\bsrc/map[a-z_]*\.(c|h|l|y)\b' "$file" || continue
  checked=$((checked + 1))
  if ! grep -qF "$COPYRIGHT" "$file" || ! grep -qF "$PERMISSION" "$file"; then
    echo "ERROR: ${file#"$ROOT"/} follows MapServer's source and does not hold its notice" >&2
    failures=$((failures + 1))
  fi
done < <(find "$ROOT/crates" "$ROOT/fuzz" -name '*.rs' -not -path '*/target/*' -print0 2>/dev/null)

if ((failures)); then
  echo "Notice check FAILED ($failures of $checked files that follow MapServer's source)." >&2
  exit 1
fi
echo "Notice check passed ($checked files that follow MapServer's source)."
exit 0
