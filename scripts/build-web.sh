#!/usr/bin/env bash
# Builds the WebAssembly bundle of the browser bindings into target/web/
# (OPS-pinned-stable-rust-toolchain): a build with the size-optimized `wasm` profile for
# wasm32-unknown-unknown, then the JavaScript glue of the wasm-bindgen CLI, which
# scripts/install-tools.sh installs at the crate's version. The dev server serves the result under
# /pkg/; scripts/check-rust.sh measures it.
#
# Usage: bash scripts/build-web.sh [--root <dir>]
# Exit code 0 when the bundle is built, 2 on an argument it does not read or a root that is no
# directory, else the failing step's.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/build-web.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"

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
