#!/usr/bin/env bash
# Builds the WebAssembly bundle of the browser bindings into target/web/ (ADR-0038): a build with
# the size-optimized `wasm` profile for wasm32-unknown-unknown, then the JavaScript glue of the
# wasm-bindgen CLI, which scripts/install-tools.sh installs at the crate's version. The dev server
# serves the result under /pkg/; scripts/check-rust.sh measures it.
#
# Usage: bash scripts/build-web.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo build --locked --profile wasm --target wasm32-unknown-unknown -p forager-bindings-wasm
# A Dev Container just created may still be compiling the tools in its postCreateCommand, and one
# created before them lacks them; install-tools.sh installs what is missing, and a cargo install
# running beside it waits on cargo's lock and then finds the version installed.
if ! command -v wasm-bindgen > /dev/null 2>&1; then
  echo "build-web: wasm-bindgen not found; running scripts/install-tools.sh" >&2
  bash scripts/install-tools.sh
fi
wasm-bindgen --target web --out-dir target/web \
  target/wasm32-unknown-unknown/wasm/forager_bindings_wasm.wasm
