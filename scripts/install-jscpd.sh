#!/usr/bin/env bash
# Installs jscpd at the version scripts/sensor-duplication.sh runs (ADR-draft-sensors-and-duplication):
# the static (musl) Linux release binary of kucherenko/jscpd for this machine's architecture,
# verified against the SHA-256 the release's checksums.txt lists for the asset, which is pinned
# here, into a directory on PATH, ~/.local/bin unless JSCPD_BIN_DIR names another. A jscpd
# already at the version there is kept. scripts/install-tools.sh runs it in the Dev Container
# (ADR-0009), and the sensors job of CI runs it before the duplication sensor.
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

if [[ "$("$BIN_DIR/jscpd" --version 2> /dev/null)" == "jscpd $JSCPD_VERSION" ]]; then
  echo "install-jscpd: jscpd $JSCPD_VERSION is installed in $BIN_DIR"
else
  name="jscpd-$asset.tar.gz"
  url="https://github.com/kucherenko/jscpd/releases/download/v$JSCPD_VERSION/$name"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT
  curl --fail --silent --show-error --location --output "$work/$name" "$url"
  echo "$sha256  $work/$name" | sha256sum --check --quiet -
  tar -xzf "$work/$name" -C "$work" jscpd
  mkdir -p "$BIN_DIR"
  install -m 0755 "$work/jscpd" "$BIN_DIR/jscpd"
  echo "install-jscpd: installed jscpd $JSCPD_VERSION in $BIN_DIR"
fi

# A jscpd of another version earlier on PATH would be the one the sensor runs.
found="$(command -v jscpd || true)"
if [[ "$found" != "$BIN_DIR/jscpd" ]]; then
  echo "install-jscpd: note: PATH finds ${found:-no jscpd} before $BIN_DIR/jscpd" >&2
fi
