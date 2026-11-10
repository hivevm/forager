#!/usr/bin/env bash
# Installs ShellCheck at the version scripts/check-shell.sh holds the shell lint to (ADR-0007):
# the release binary of koalaman/shellcheck for this machine's architecture, verified against the
# SHA-256 GitHub records for the release asset, which is pinned here, into a directory on PATH,
# ~/.local/bin unless SHELLCHECK_BIN_DIR names another. A ShellCheck already at the version there
# is kept. scripts/install-tools.sh runs it in the Dev Container (ADR-0009), and the shell job of
# CI runs it before the lint.
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

if grep -qx "version: $SHELLCHECK_VERSION" <<< "$("$BIN_DIR/shellcheck" --version 2> /dev/null)"; then
  echo "install-shellcheck: shellcheck $SHELLCHECK_VERSION is installed in $BIN_DIR"
else
  name="shellcheck-v$SHELLCHECK_VERSION.$asset.tar.xz"
  url="https://github.com/koalaman/shellcheck/releases/download/v$SHELLCHECK_VERSION/$name"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  curl --fail --silent --show-error --location --output "$work/$name" "$url"
  echo "$sha256  $work/$name" | sha256sum --check --quiet -
  tar -xJf "$work/$name" -C "$work"
  mkdir -p "$BIN_DIR"
  install -m 0755 "$work/shellcheck-v$SHELLCHECK_VERSION/shellcheck" "$BIN_DIR/shellcheck"
  echo "install-shellcheck: installed shellcheck $SHELLCHECK_VERSION in $BIN_DIR"
fi

# A ShellCheck of another version earlier on PATH would be the one the lint runs.
found="$(command -v shellcheck || true)"
if [[ "$found" != "$BIN_DIR/shellcheck" ]]; then
  echo "install-shellcheck: note: PATH finds ${found:-no shellcheck} before $BIN_DIR/shellcheck" >&2
fi
