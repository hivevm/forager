#!/usr/bin/env bash
# Runs the fuzz targets under fuzz/ (Q-3; OPS-pinned-stable-rust-toolchain,
# REN-agg-rust-rasterizer), each for a number of seconds, on the nightly toolchain cargo-fuzz needs.
# Both are pinned here. The targets are the [[bin]] entries of fuzz/Cargo.toml, read from there, so
# that no target the manifest builds is skipped; a target without seeds under fuzz/seeds/<target>
# fails the run before anything is fuzzed; scripts/test-fuzz.sh tests how the targets are read,
# refused, and run. A crash, a panic, a hang longer than 10 s, or more than 2 GiB of memory fails
# the target, and the input lands in fuzz/artifacts/<target>/. A target that fails does not stop the
# others: every target runs for its time, and the run fails at the end, naming each target that
# failed.
# Before fuzzing, cargo deny checks the advisories and sources of the fuzz workspace, which the
# main check does not see; its licences are not checked, since the harness is never distributed.
# The weekly workflow fuzz.yml runs it, after install-tools.sh.
#
# An argument that is no positive whole number of seconds is refused with the usage, before
# anything is read or installed. A target named after the seconds is fuzzed alone; a name the
# manifest does not list is refused with the names it lists, before anything is installed.
#
# '--replay <target> <input>' runs the target once on the input file, as a fix's session checks
# a crash's input, on the pinned toolchain and with the limits of a run, and exits with the
# target's result. It installs what a run installs, but runs no cargo deny, which guards the
# weekly run's supply chain and not one input's replay, and needs no seeds. A target the manifest
# does not list and an input file that does not exist are refused before anything is installed.
#
# Usage: bash scripts/fuzz.sh [--root <dir>] [seconds per target, default 300 [target]]
#        bash scripts/fuzz.sh [--root <dir>] --replay <target> <input>
# The root, this repository unless --root names another, holds fuzz/ and is where it runs; an
# input to replay is read from where the script is called. An argument it does not read, such as
# an option, and a root that is no directory exit 2.
# Verifies: Q-3
set -euo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# The directory the script is called from, which an input to replay is read from: enter_root
# changes to the root.
caller="$PWD"
# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/fuzz.sh [--root <dir>] {[seconds [target]] | --replay <target> <input>}"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
set -- "${ARGS[@]}"

NIGHTLY="nightly-2026-10-20"
CARGO_FUZZ_VERSION="0.13.2"
LIMITS=(-timeout=10 -rss_limit_mb=2048)

usage() {
  echo "usage: bash scripts/fuzz.sh [--root <dir>] [seconds per target, default 300 [target]]" >&2
  echo "       bash scripts/fuzz.sh [--root <dir>] --replay <target> <input>" >&2
  exit 2
}

REPLAY=""
if [[ "${1:-}" == --replay ]]; then
  (($# == 3)) || usage
  rest_args "${BASH_SOURCE[0]}" "$usage" 2 "${@:2}"
  ONLY="$2"
  # The input as the caller named it, from wherever the script is called; it runs from $ROOT.
  input="$3"
  [[ "$input" == /* ]] || input="$caller/$input"
  if [[ ! -f "$input" ]]; then
    echo "the input to replay, $3, is no file" >&2
    exit 1
  fi
  REPLAY="$(cd "$(dirname "$input")" && pwd)/$(basename "$input")"
else
  (($# <= 2)) || usage
  rest_args "${BASH_SOURCE[0]}" "$usage" 2 "$@"
  SECONDS_PER_TARGET="${1-300}"
  ONLY="${2:-}"
  if [[ ! "$SECONDS_PER_TARGET" =~ ^[1-9][0-9]*$ ]]; then
    echo "the seconds per target are a positive whole number, not '$SECONDS_PER_TARGET'" >&2
    usage
  fi
fi

# The name of every binary target of the fuzz manifest, as cargo reads it, so that no way of
# writing a [[bin]] entry hides a target. The names are read into a variable first, so that a
# failing cargo or jq stops the run with its own message.
names="$(
  cargo metadata --manifest-path fuzz/Cargo.toml --no-deps --format-version 1 --offline |
    jq -r '.packages[].targets[] | select(.kind | index("bin")) | .name'
)"
targets=()
if [ -n "$names" ]; then
  mapfile -t targets <<< "$names"
fi
if [ "${#targets[@]}" -eq 0 ]; then
  echo "fuzz/Cargo.toml names no [[bin]] target" >&2
  exit 1
fi
if [ -n "$ONLY" ]; then
  if ! grep -qxF -- "$ONLY" <<< "$names"; then
    echo "fuzz/Cargo.toml lists no fuzz target $ONLY; it lists: ${targets[*]}" >&2
    exit 1
  fi
  targets=("$ONLY")
fi
if [ -z "$REPLAY" ]; then
  for target in "${targets[@]}"; do
    if [ ! -d "fuzz/seeds/$target" ]; then
      echo "the fuzz target $target has no seeds under fuzz/seeds/$target" >&2
      exit 1
    fi
  done
fi

rustup toolchain install "$NIGHTLY" --profile minimal
if ! grep -qF " $CARGO_FUZZ_VERSION" <<< "$(cargo fuzz --version 2> /dev/null)"; then
  cargo install --locked cargo-fuzz --version "$CARGO_FUZZ_VERSION"
fi
if [ -n "$REPLAY" ]; then
  exec cargo "+$NIGHTLY" fuzz run "$ONLY" "$REPLAY" -- "${LIMITS[@]}"
fi
cargo deny --manifest-path fuzz/Cargo.toml --config deny.toml --locked check advisories sources

failed=()
for target in "${targets[@]}"; do
  printf '\n--> fuzz %s for %s s\n' "$target" "$SECONDS_PER_TARGET"
  mkdir -p "fuzz/corpus/$target"
  if ! cargo "+$NIGHTLY" fuzz run "$target" "fuzz/corpus/$target" "fuzz/seeds/$target" -- \
    -max_total_time="$SECONDS_PER_TARGET" "${LIMITS[@]}"; then
    failed+=("$target")
  fi
done

if [ "${#failed[@]}" -gt 0 ]; then
  echo >&2
  for target in "${failed[@]}"; do
    where="a crash's input is under fuzz/artifacts/$target, the cause otherwise in the output above"
    echo "the fuzz target $target failed; $where" >&2
  done
  exit 1
fi
