#!/usr/bin/env bash
# Installs ShellCheck at the version scripts/check-shell.sh holds the shell lint to (OPS-pinned-stable-rust-toolchain):
# the release binary of koalaman/shellcheck for this machine's architecture, verified against the
# SHA-256 GitHub records for the release asset, which is pinned here, into a directory on PATH,
# ~/.local/bin unless SHELLCHECK_BIN_DIR names another. A ShellCheck already at the version there
# is kept. scripts/install-tools.sh runs it in the Dev Container (OPS-dev-container-adds-rust), and the shell job of
# CI runs it before the lint. The download, the check, and the install are scripts/lib/install.sh.
#
# Usage: bash scripts/install-shellcheck.sh
set -euo pipefail

SHELLCHECK_VERSION="0.11.0"
BIN_DIR="${SHELLCHECK_BIN_DIR:-$HOME/.local/bin}"

# The SHA-256 of each release asset, as GitHub records it for the release v0.11.0.
case "$(uname -s).$(uname -m)" in
  Linux.x86_64)
    asset="linux.x86_64"
    sha256="8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198"
    ;;
  Linux.aarch64 | Linux.arm64)
    asset="linux.aarch64"
    sha256="12b331c1d2db6b9eb13cfca64306b1b157a86eb69db83023e261eaa7e7c14588"
    ;;
  *)
    echo "install-shellcheck: no pinned ShellCheck for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

# shellcheck source-path=SCRIPTDIR source=lib/install.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/install.sh"
release="https://github.com/koalaman/shellcheck/releases/download/v$SHELLCHECK_VERSION"
install_release shellcheck "$SHELLCHECK_VERSION" "$BIN_DIR" "version: $SHELLCHECK_VERSION" \
  "$release/shellcheck-v$SHELLCHECK_VERSION.$asset.tar.xz" "$sha256" \
  "shellcheck-v$SHELLCHECK_VERSION/shellcheck"
