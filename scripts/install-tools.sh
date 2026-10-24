#!/usr/bin/env bash
# Installs what scripts/check-rust.sh needs beyond rustup (ADR-0008): the toolchain pinned in
# rust-toolchain.toml, cargo-deny at the version named below, and the wasm-bindgen CLI at exactly
# the version of the wasm-bindgen crate in Cargo.lock, which the generated glue must match. A tool
# already at its version is skipped. The Dev Container runs this after creation (ADR-0010) and CI
# before the build, with ~/.cargo/bin cached.
#
# Usage: bash scripts/install-tools.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_DENY_VERSION="0.20.2"

if ! command -v rustup > /dev/null 2>&1; then
  echo "install-tools: rustup is required (https://rustup.rs)" >&2
  exit 1
fi

cd "$ROOT"
# Without arguments, rustup installs the toolchain rust-toolchain.toml names, with its components
# and targets.
rustup toolchain install

# install_tool <crate> <version> <binary> — cargo install, unless <binary> --version already
# reports <version>.
install_tool() {
  local crate="$1" version="$2" binary="$3"
  if "$binary" --version 2> /dev/null | grep -qF " $version"; then
    echo "install-tools: $binary $version is installed"
    return
  fi
  cargo install --locked "$crate" --version "$version"
}

install_tool cargo-deny "$CARGO_DENY_VERSION" cargo-deny

# The version of the wasm-bindgen crate in Cargo.lock; empty while no crate depends on it.
wasm_bindgen_version="$(awk '
  $0 == "name = \"wasm-bindgen\"" { found = 1; next }
  found && /^version = / { gsub(/"/, "", $3); print $3; exit }
' Cargo.lock)"
if [[ -n "$wasm_bindgen_version" ]]; then
  install_tool wasm-bindgen-cli "$wasm_bindgen_version" wasm-bindgen
else
  echo "install-tools: no wasm-bindgen in Cargo.lock; the wasm-bindgen CLI is not needed yet"
fi
