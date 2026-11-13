#!/usr/bin/env bash
#
# Packages the two binary artefacts of Forager under target/package/, and holds each to the
# notices it must carry: W. Randolph Franklin's pnpoly licence asks in its second condition that a
# redistribution in binary form reproduces his notice, and THIRD-PARTY-NOTICES.md holds it with
# MapServer's.
#
# - The CLI's archive, forager-<version>-<host>.tar.gz: the release build of the binary
#   `forager` for the host's target.
# - The browser bindings' package, forager-web-<version>.tar.gz: the bundle scripts/build-web.sh
#   writes to target/web/, the wasm and its JavaScript glue.
#
# Each archive holds one directory named as the archive, without .tar.gz. That directory holds
# THIRD-PARTY-NOTICES.md, LICENSE, and LICENSE-APACHE beside the build's files. The check then
# reads each archive as it was written: an archive that lacks one of the three files, or whose
# THIRD-PARTY-NOTICES.md lacks Franklin's notice or MapServer's copyright line
# (scripts/lib/notice.sh, which scripts/check-notice.sh shares), fails, named.
#
# Nothing is published and nothing leaves the machine: the workspace keeps `publish = false`, and
# no workflow builds a release. A distribution of a binary starts from these archives.
#
# Usage:
#     scripts/package.sh                    build both artefacts, then check them
#     scripts/package.sh --check <archive>...   check the archives only
# scripts/test-package.sh checks fixture archives with --check.
# Exit code 0 when every artefact carries the notices, 1 when one does not or a build fails, 2 on
# a usage error.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source-path=SCRIPTDIR source=lib/notice.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/notice.sh"

# shellcheck source-path=SCRIPTDIR source=lib/package.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/package.sh"

if [[ "${1:-}" == --check ]]; then
  shift
  (($#)) || { echo "package: --check needs at least one archive" >&2; exit 2; }
  check_archives "$@"
fi
(($# == 0)) || { echo "package: unknown argument '$1' — usage: scripts/package.sh [--check <archive>...]" >&2; exit 2; }

cd "$ROOT" || exit 1
version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)"
host="$(rustc -vV | sed -n 's/^host: //p')"
[[ -n "$version" && -n "$host" ]] || { echo "package: no version or host target found" >&2; exit 1; }
out="$ROOT/target/package"
mkdir -p "$out"

cargo build --locked --release --target-dir "$ROOT/target" -p forager-cli || exit 1
bin="$(mktemp -d)"
trap 'rm -rf "$bin"' EXIT
cp target/release/forager "$bin/" || exit 1
cli="$out/forager-$version-$host.tar.gz"
pack "$bin" "$cli" || { echo "package: the CLI's archive could not be written" >&2; exit 1; }

# A fresh build, so that no file an earlier build left in target/web/ is packed.
rm -rf target/web
bash scripts/build-web.sh || exit 1
web="$out/forager-web-$version.tar.gz"
pack target/web "$web" || { echo "package: the browser bindings' package could not be written" >&2; exit 1; }

check_archives "$cli" "$web"
