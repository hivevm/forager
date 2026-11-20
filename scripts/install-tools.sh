#!/usr/bin/env bash
# Installs what scripts/check-rust.sh needs beyond rustup (OPS-pinned-stable-rust-toolchain): the toolchain pinned in
# rust-toolchain.toml, cargo-deny at the version named below, and the wasm-bindgen CLI at exactly
# the version of the wasm-bindgen crate in Cargo.lock, which the generated glue must match; and
# ShellCheck at the version scripts/install-shellcheck.sh pins, for the shell lint (OPS-pinned-stable-rust-toolchain); and
# jscpd at the version scripts/install-jscpd.sh pins, for the duplication sensor
# (OPS-pinned-stable-rust-toolchain). A tool already at its version is skipped. The Dev Container
# runs this after creation (OPS-dev-container-adds-rust) and CI before the build, with ~/.cargo/bin cached.
#
# Usage: bash scripts/install-tools.sh [--root <dir>]
# The toolchain and the wasm-bindgen version are read from the root, this repository unless --root
# names another; ShellCheck and jscpd come from the installers beside this script.
# Exit code 0 when every tool is installed, 2 on an argument it does not read or a root that is no
# directory, 1 otherwise.
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# The installers beside this script, found before enter_root changes the directory.
SCRIPTS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/install-tools.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"
CARGO_DENY_VERSION="0.20.2"

if ! command -v rustup > /dev/null 2>&1; then
  echo "install-tools: rustup is required (https://rustup.rs)" >&2
  exit 1
fi

# Without arguments, rustup installs the toolchain rust-toolchain.toml names, with its components
# and targets.
rustup toolchain install

# install_tool <crate> <version> <binary> — cargo install, unless <binary> --version already
# reports <version>.
install_tool() {
  local crate="$1" version="$2" binary="$3"
  if grep -qF " $version" <<< "$("$binary" --version 2> /dev/null)"; then
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

bash "$SCRIPTS/install-shellcheck.sh"
bash "$SCRIPTS/install-jscpd.sh"
