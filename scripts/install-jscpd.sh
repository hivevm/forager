#!/usr/bin/env bash
# Installs jscpd at the version scripts/sensor-duplication.sh runs (ADR-0007):
# the static (musl) Linux release binary of kucherenko/jscpd for this machine's architecture,
# verified against the SHA-256 the release's checksums.txt lists for the asset, which is pinned
# here, into a directory on PATH, ~/.local/bin unless JSCPD_BIN_DIR names another. A jscpd
# already at the version there is kept. scripts/install-tools.sh runs it in the Dev Container
# (ADR-0009). In CI, the sensors job runs it before the duplication sensor and its self-test, and
# the docs and shell jobs before the self-tests that run that self-test. The download, the check,
# and the install are scripts/lib/install.sh.
#
# Usage: bash scripts/install-jscpd.sh
set -euo pipefail

JSCPD_VERSION="5.4.3"
BIN_DIR="${JSCPD_BIN_DIR:-$HOME/.local/bin}"

# The SHA-256 of each release asset, as checksums.txt of the release v5.4.3 lists it.
case "$(uname -s).$(uname -m)" in
  Linux.x86_64)
    asset="linux-x64-musl"
    sha256="291452d6005e3fa17b531552bf76f87ff85bc4ae31533e0c17a7997f84079868"
    ;;
  Linux.aarch64 | Linux.arm64)
    asset="linux-arm64-musl"
    sha256="f363875348817f8e999ca090266b1265ba29322d6b5d1f6280aa0d136ab1ca9e"
    ;;
  *)
    echo "install-jscpd: no pinned jscpd for $(uname -s) $(uname -m)" >&2
    exit 1
    ;;
esac

# shellcheck source-path=SCRIPTDIR source=lib/install.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/install.sh"
install_release jscpd "$JSCPD_VERSION" "$BIN_DIR" "jscpd $JSCPD_VERSION" \
  "https://github.com/kucherenko/jscpd/releases/download/v$JSCPD_VERSION/jscpd-$asset.tar.gz" \
  "$sha256" jscpd
