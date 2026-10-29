#!/usr/bin/env bash
# Builds map2img of the reference release of MapServer, the instrument of the noise measurement
# of G-6 (ADR-0027): from the commit scripts/fetch-msautotest.sh pins, which this script reads and
# never names itself, with MapServer's default build options, SVG symbols through librsvg, and
# curl, as MapServer's own CI builds the references, and the Ubuntu archive's packages below, into
# target/mapserver/<commit>/, and prints the path of map2img. It adds no apt source. It runs on a
# GitHub-hosted runner only, where .github/workflows/noise.yml calls it, and refuses anywhere else,
# the dev container included.
#
# Into the job summary it writes the CMake options of the build and, for each package whose
# library map2img links, the version installed and the origin apt-cache policy reports, so that a
# shift of the noise between two runs can be traced to the libraries, and a library from a source
# other than the archive is seen. A build already in target/ is kept, as the workflow restores it
# from its cache; the packages are installed on every run, as map2img links them.
#
# Usage: bash scripts/build-mapserver.sh    (prints the path of map2img)
set -euo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

if [[ "${GITHUB_ACTIONS:-}" != "true" ]]; then
  echo "build-mapserver: runs on a GitHub-hosted runner only, never in the dev container (ADR-0027)" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PIN="$ROOT/scripts/fetch-msautotest.sh"
TAG="$(sed -n 's/^TAG="\([^"]*\)"$/\1/p' "$PIN")"
COMMIT="$(sed -n 's/^COMMIT="\([^"]*\)"$/\1/p' "$PIN")"
REMOTE="$(sed -n 's/^REMOTE="\([^"]*\)"$/\1/p' "$PIN")"
if [[ -z "$TAG" || -z "$COMMIT" || -z "$REMOTE" ]]; then
  echo "build-mapserver: scripts/fetch-msautotest.sh assigns no TAG, COMMIT, or REMOTE" >&2
  exit 1
fi

# What the default options of MapServer's CMakeLists.txt, WITH_RSVG, and WITH_CURL need, and the
# tools of its build; the curl variant is the one MapServer's CI installs.
PACKAGES=(
  bison cmake flex g++ make
  libcairo2-dev libcurl4-gnutls-dev libfcgi-dev libfreetype-dev libfribidi-dev libgdal-dev libgeos-dev libgif-dev
  libharfbuzz-dev libjpeg-dev libpcre2-dev libpng-dev libpq-dev libproj-dev libprotobuf-c-dev
  librsvg2-dev libxml2-dev protobuf-c-compiler
)
sudo apt-get update -q >&2
sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -q --no-install-recommends "${PACKAGES[@]}" >&2

DIR="$ROOT/target/mapserver/$COMMIT"
MAP2IMG="$DIR/build/map2img"
if [[ ! -f "$DIR/.complete" ]]; then
  rm -rf "$DIR"
  git -c advice.detachedHead=false clone -q --depth 1 --branch "$TAG" "$REMOTE" "$DIR/src" >&2
  checked_out="$(git -C "$DIR/src" rev-parse HEAD)"
  if [[ "$checked_out" != "$COMMIT" ]]; then
    echo "build-mapserver: the checkout is $checked_out, the pin is $COMMIT" >&2
    rm -rf "$DIR"
    exit 1
  fi
  cmake -S "$DIR/src" -B "$DIR/build" -DCMAKE_BUILD_TYPE=Release -DWITH_RSVG=ON \
    -DWITH_CURL=ON >&2
  cmake --build "$DIR/build" --target map2img --parallel "$(nproc)" >&2
  touch "$DIR/.complete"
fi

if [[ -n "${GITHUB_STEP_SUMMARY:-}" ]]; then
  {
    echo "## MapServer $TAG ($COMMIT)"
    echo
    echo "CMake options of the build:"
    echo
    echo '```'
    grep -E '^WITH_[A-Z0-9_]+:BOOL=' "$DIR/build/CMakeCache.txt"
    echo '```'
    echo
    echo "| Package | Version | Origin |"
    echo "| --- | --- | --- |"
    # Each library map2img links, and the package that holds it; under merged /usr a library may
    # be registered under /lib or /usr/lib. Libraries of the build itself belong to no package.
    ldd "$MAP2IMG" | awk '$2 == "=>" && $3 ~ /^\// {print $3}' | while read -r lib; do
      real="$(readlink -f "$lib")"
      dpkg -S "$real" 2>/dev/null || dpkg -S "${real#/usr}" 2>/dev/null \
        || echo "(no package): $real"
    done | sed 's/: .*//' | sort -u | while read -r package; do
      if [[ "$package" == "(no package)" ]]; then
        echo "| $package | | |"
        continue
      fi
      version="$(dpkg-query -W -f '${Version}' "$package")"
      origin="$(apt-cache policy "$package" | awk '/^ \*\*\* /{getline; print $2, $3; exit}')"
      echo "| $package | $version | $origin |"
    done
  } >>"$GITHUB_STEP_SUMMARY"
fi

echo "$MAP2IMG"
