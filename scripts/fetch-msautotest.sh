#!/usr/bin/env bash
# Fetches the msautotest suite of the reference release of MapServer: a sparse checkout of its
# mapfiles and the files they read, into target/msautotest/<commit>/, and prints the suite's
# directory. The pin below is the one place that names the reference release (specification,
# *Constraints*: "pinned by tag"); the comments that cite MapServer's source refer to it. Moving
# the pin is a deliberate change with a full conformance run, and changes the key of the CI cache.
#
# The tag must still name the pinned commit: the check asks the remote on every run, so a retagged
# release fails here, naming both, instead of passing with other files. The suite is never
# vendored; its licence is MapServer's (MIT-style, OSGeo and the University of Minnesota).
#
# Usage: bash scripts/fetch-msautotest.sh    (prints the directory of the suite)
set -euo pipefail

TAG="rel-8-6-6"
COMMIT="7515cccfe75fb7554ea45bc87ba5eb4b6126db5d"
REMOTE="https://github.com/MapServer/MapServer.git"

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIR="$ROOT/target/msautotest/$COMMIT"

tagged="$(git ls-remote --tags "$REMOTE" "refs/tags/$TAG^{}" | cut -f1)"
if [[ "$tagged" != "$COMMIT" ]]; then
  echo "fetch-msautotest: tag $TAG names ${tagged:-no commit}, the pin is $COMMIT" >&2
  exit 1
fi

if [[ ! -f "$DIR/.complete" ]]; then
  rm -rf "$DIR"
  git -c advice.detachedHead=false clone -q --depth 1 --branch "$TAG" --filter=blob:none --sparse "$REMOTE" "$DIR" >&2
  git -C "$DIR" sparse-checkout set --no-cone \
    '/msautotest/**/*.map' '/msautotest/**/*.MAP' '/msautotest/**/*.inc' \
    '/msautotest/**/*.include' '/msautotest/**/*.sym' '/msautotest/**/*.txt' \
    '/msautotest/**/*symbolset' >&2
  checked_out="$(git -C "$DIR" rev-parse HEAD)"
  if [[ "$checked_out" != "$COMMIT" ]]; then
    echo "fetch-msautotest: the checkout is $checked_out, the pin is $COMMIT" >&2
    rm -rf "$DIR"
    exit 1
  fi
  touch "$DIR/.complete"
fi
echo "$DIR/msautotest"
