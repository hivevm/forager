#!/usr/bin/env bash
# The mutation testing of ADR-0007: cargo-mutants changes the code one small edit at a time and
# reports each edit no test notices — a test that runs without deciding. A sensor: it reports
# what survived and fails only when the run itself breaks, not on what it counts. With a base,
# only the Rust code changed since it is mutated: the lines changed from the merge base to the
# working tree. The review runs it with the branch the change leaves from, so that a branch no
# test decides is reported before the merge, not a week after it. The weekly workflow
# mutants.yml runs it with main as it stood a week before, so that each week's code is mutated
# once. Without a base it mutates the whole workspace, some 12,000 mutants and far more hours
# than a runner's job may take; a shard, such as 1/50, cuts it to a share that fits. A change
# that touches no Rust code has nothing to mutate. Files git does not track are not part of the
# diff: a new file is mutated once it is added.
#
# Each mutant's tests stop after 300 seconds. cargo-mutants would set five times the tests'
# baseline run, some 1,000 seconds on a runner, and a mutant that turns a parser's step into a
# loop spent all of it. 300 seconds stay above the 205 the whole workspace's tests take there.
#
# Usage: bash scripts/mutants.sh [--base <revision>] [shard, e.g. 1/4; default all]
#   e.g. bash scripts/mutants.sh --base origin/main
set -uo pipefail
# The diff is taken in this repository, never in one a caller names through git's variables
# (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_MUTANTS_VERSION="27.1.0"
USAGE="usage: bash scripts/mutants.sh [--base <revision>] [shard]"

BASE=""
SHARD=""
while (($#)); do
  case "$1" in
    --base | --base=*)
      [[ -z "$BASE" ]] || { echo "mutants: more than one base; $USAGE" >&2; exit 1; }
      ;;&
    --base)
      [[ -n "${2:-}" ]] || { echo "mutants: --base needs a revision; $USAGE" >&2; exit 1; }
      BASE="$2"
      shift 2
      ;;
    --base=*)
      BASE="${1#--base=}"
      [[ -n "$BASE" ]] || { echo "mutants: --base needs a revision; $USAGE" >&2; exit 1; }
      shift
      ;;
    -*) echo "mutants: unknown option '$1'; $USAGE" >&2; exit 1 ;;
    *)
      [[ -z "$SHARD" ]] || { echo "mutants: more than one shard; $USAGE" >&2; exit 1; }
      SHARD="$1"
      shift
      ;;
  esac
done

cd "$ROOT" || exit 1
args=(--workspace --test-workspace=false --no-shuffle --timeout 300 --output target/mutants)

if [[ -n "$BASE" ]]; then
  if ! git rev-parse --verify --quiet "$BASE^{commit}" > /dev/null; then
    echo "mutants: the base '$BASE' does not resolve to a commit" >&2
    exit 1
  fi
  if ! fork="$(git merge-base "$BASE" HEAD)"; then
    echo "mutants: the base '$BASE' shares no history with HEAD" >&2
    exit 1
  fi
  diff_file=target/mutants.diff
  mkdir -p target || exit 1
  # The diff cargo-mutants reads, whatever the user's git config: no colour, no external or
  # text-converting diff, and the a/ b/ prefixes it strips.
  git diff --no-color --no-ext-diff --no-textconv --src-prefix=a/ --dst-prefix=b/ "$fork" \
    -- '*.rs' > "$diff_file" || exit 1
  if [[ ! -s "$diff_file" ]]; then
    echo "mutants: no Rust code changed since '$BASE' — nothing to mutate"
    exit 0
  fi
  args+=(--in-diff "$diff_file")
fi
# A shard takes every n-th mutant, not a slice: cargo-mutants 27.1.0 slices by default, which
# put all of one crate's mutants, and its slow tests, in one shard.
[[ -n "$SHARD" ]] && args+=(--shard "$SHARD" --sharding round-robin)

if ! grep -qF " $CARGO_MUTANTS_VERSION" <<< "$(cargo mutants --version 2> /dev/null)"; then
  cargo install --locked cargo-mutants --version "$CARGO_MUTANTS_VERSION" || exit 1
fi

# cargo-mutants 27.1.0 makes its output directory but not that directory's parent, and the
# weekly workflow's fresh checkout has no target/: the script makes the output on every path.
mkdir -p target/mutants || exit 1
# A run that finds nothing to mutate writes no report; an earlier run's must not stand for it.
report=target/mutants/mutants.out
rm -rf "$report" || exit 1
cargo mutants "${args[@]}"
status=$?

# cargo-mutants: 0 every mutant caught, 2 some missed, 3 some timed out — the findings this
# sensor reports; anything else is a broken run.
if [[ -f "$report/missed.txt" ]]; then
  printf '\nMutants no test caught (%s):\n' "$(wc -l < "$report/missed.txt")"
  cat "$report/missed.txt"
fi
case "$status" in
  0 | 2 | 3) exit 0 ;;
  *) echo "mutants: the run failed (exit $status)" >&2; exit 1 ;;
esac
