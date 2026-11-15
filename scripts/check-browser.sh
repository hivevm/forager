#!/usr/bin/env bash
#
# Check: the browser example in a real browser (G-14, ADR-draft-browser-test-over-webdriver).
#
# Runs the browser test crates/dev-server/tests/browser.rs alone. It opens examples/web/ in
# headless Chrome over ChromeDriver, served by the dev server with the bundle of
# scripts/build-web.sh, and compares the canvas and the list of diagnostics with the native render.
#
# Where FORAGER_BROWSER_TEST=required, as in the browser job of .github/workflows/checks.yml, this
# script builds the bundle first, and a missing ChromeDriver (CHROMEWEBDRIVER, else PATH), Chrome,
# or bundle fails the test. Elsewhere, as in the Dev Container, which has no browser (ADR-0009),
# the test's cases return at once with a note on stderr, and the bundle is not built.
#
# Usage: bash scripts/check-browser.sh
# Exit code 0 when the test passes or does not run, non-zero otherwise.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "${FORAGER_BROWSER_TEST:-}" == required ]]; then
  bash scripts/build-web.sh
fi
cargo test --locked -p forager-dev-server --test browser
