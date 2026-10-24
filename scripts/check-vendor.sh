#!/usr/bin/env bash
#
# Check: the vendored JavaScript of the browser example (ADR-0021).
#
# The example loads third-party code only from copies under examples/web/vendor/, and those copies
# are the published bytes. This check fails when:
#
#   1. a file listed in examples/web/vendor/SHA256SUMS is missing or its SHA-256 differs;
#   2. a file under examples/web/vendor/ is not listed there;
#   3. a 'src' or 'href' of examples/web/index.html names another origin ('http:', 'https:', or
#      '//'), so that the page would load code the repository does not hold.
#
# Pure bash + coreutils (sha256sum, or shasum on macOS). Usage:
#     scripts/check-vendor.sh [repository root]
# The root defaults to this repository; scripts/test-check-vendor.sh passes fixtures.
# Exit code 0 when everything matches, 1 otherwise, 2 on a root that is no directory.

set -uo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Vendor check: '${1:-}' is not a directory." >&2; exit 2; }
VENDOR="$ROOT/examples/web/vendor"
PAGE="$ROOT/examples/web/index.html"

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$@"; }
else
  sha256() { shasum -a 256 "$@"; }
fi

failures=0
fail() { echo "ERROR: $*" >&2; failures=$((failures + 1)); }

if [[ -d "$VENDOR" ]]; then
  if [[ -f "$VENDOR/SHA256SUMS" ]]; then
    (cd "$VENDOR" && sha256 --check --quiet SHA256SUMS) \
      || fail "a vendored file differs from examples/web/vendor/SHA256SUMS"
    while IFS= read -r file; do
      awk -v f="$file" '$2 == f { found = 1 } END { exit !found }' "$VENDOR/SHA256SUMS" \
        || fail "examples/web/vendor/$file is not listed in SHA256SUMS"
    done < <(cd "$VENDOR" && find . -type f ! -name SHA256SUMS | sed 's|^\./||' | sort)
  else
    fail "examples/web/vendor/ has no SHA256SUMS"
  fi
fi

if [[ -f "$PAGE" ]]; then
  while IFS= read -r ref; do
    fail "examples/web/index.html loads from another origin: $ref"
  done < <(grep -oiE "(src|href)=[\"']?(https?:|//)[^\"' >]*" "$PAGE")
fi

if ((failures)); then
  exit 1
fi
echo "Vendor check passed."
