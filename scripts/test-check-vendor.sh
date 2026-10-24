#!/usr/bin/env bash
#
# Self-test of scripts/check-vendor.sh. Verifies: ADR-0021
#
# Each case writes a fixture repository with a vendored file, its checksum list, and a page, runs
# the check against it, and asserts the exit code and the line that names the problem. The
# untouched fixture passes; every other case is one way the vendored code or the page can drift.
#
# Usage:
#     scripts/test-check-vendor.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHECK="$ROOT/scripts/check-vendor.sh"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$@"; }
else
  sha256() { shasum -a 256 "$@"; }
fi

# repo <name> — a fixture with one vendored library, listed, and a page that loads it, printed as
# its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/examples/web/vendor/lib"
  printf 'window.L = {};\r\n' > "$dir/examples/web/vendor/lib/lib.js"
  (cd "$dir/examples/web/vendor" && sha256 lib/lib.js > SHA256SUMS)
  printf '<link rel="stylesheet" href="vendor/lib/lib.css">\n<script src="vendor/lib/lib.js"></script>\n' \
    > "$dir/examples/web/index.html"
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

expect "the listed, unchanged copy passes" "$(repo clean)" 0 "Vendor check passed."

dir="$(repo changed)"
printf 'window.L = {evil: 1};\r\n' > "$dir/examples/web/vendor/lib/lib.js"
expect "a changed vendored file fails" "$dir" 1 "differs from examples/web/vendor/SHA256SUMS"

dir="$(repo normalized)"
printf 'window.L = {};\n' > "$dir/examples/web/vendor/lib/lib.js"
expect "line endings rewritten fail" "$dir" 1 "differs from examples/web/vendor/SHA256SUMS"

dir="$(repo unlisted)"
printf 'x\n' > "$dir/examples/web/vendor/lib/extra.js"
expect "an unlisted vendored file fails" "$dir" 1 "lib/extra.js is not listed"

dir="$(repo missing)"
rm "$dir/examples/web/vendor/lib/lib.js"
expect "a listed file that is missing fails" "$dir" 1 "differs from examples/web/vendor/SHA256SUMS"

dir="$(repo nolist)"
rm "$dir/examples/web/vendor/SHA256SUMS"
expect "a vendor directory without a list fails" "$dir" 1 "has no SHA256SUMS"

dir="$(repo cdn)"
printf '<script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>\n' \
  >> "$dir/examples/web/index.html"
expect "a script from another origin fails" "$dir" 1 "another origin: src=\"https://unpkg.com"

dir="$(repo relative-origin)"
printf "<link href='//cdn.example/x.css'>\n" >> "$dir/examples/web/index.html"
expect "a protocol-relative stylesheet fails" "$dir" 1 "another origin: href='//cdn.example"

echo "Vendor check self-test: $passed passed, $failed failed."
((failed == 0))
