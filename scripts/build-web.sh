#!/usr/bin/env bash
# Builds the WebAssembly bundle of the browser bindings into target/web/ (ADR-0008): a build with
# the size-optimized `wasm` profile for wasm32-unknown-unknown, then the JavaScript glue of the
# wasm-bindgen CLI, which scripts/install-tools.sh installs at the crate's version. The dev server
# serves the result under /pkg/; scripts/check-rust.sh measures it.
#
# Usage: bash scripts/build-web.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo build --locked --profile wasm --target wasm32-unknown-unknown -p forager-bindings-wasm
wasm-bindgen --target web --out-dir target/web \
  target/wasm32-unknown-unknown/wasm/forager_bindings_wasm.wasm
