#!/usr/bin/env bash
# Compares the bits of smoothsia's results on a commit with those of the working tree: the probe
# of ticket 02 of revision-2026-11-19. It runs the example smoothsia_bits, 20,000 random parts
# from a fixed seed, on the tree and on the commit, the example copied into a temporary worktree
# of the commit, and fails where one line of their outputs differs.
#
# Usage: bash crates/symbolize/smoothsia-bits.sh <commit>
set -euo pipefail

(($# == 1)) || {
  echo "usage: bash crates/symbolize/smoothsia-bits.sh <commit>" >&2
  exit 2
}
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
EXAMPLE="crates/symbolize/examples/smoothsia_bits.rs"
work="$(mktemp -d)"
trap 'git -C "$ROOT" worktree remove --force "$work/base" > /dev/null 2>&1 || true; rm -rf "$work"' EXIT

git -C "$ROOT" worktree add --detach "$work/base" "$1" > /dev/null
mkdir -p "$work/base/crates/symbolize/examples"
cp "$ROOT/$EXAMPLE" "$work/base/$EXAMPLE"
(cd "$work/base" && cargo run --quiet --release -p forager-symbolize --example smoothsia_bits) \
  > "$work/base.txt"
(cd "$ROOT" && cargo run --quiet --release -p forager-symbolize --example smoothsia_bits) \
  > "$work/tree.txt"
lines="$(wc -l < "$work/tree.txt")"
if cmp "$work/base.txt" "$work/tree.txt"; then
  echo "the $lines cases are bit-identical on $1 and on the tree"
else
  diff "$work/base.txt" "$work/tree.txt" | head -20 >&2
  echo "the results differ between $1 and the tree" >&2
  exit 1
fi
