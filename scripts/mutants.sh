#!/usr/bin/env bash
# The mutation testing of OPS-pinned-stable-rust-toolchain: cargo-mutants changes the code one small
# edit at a time and reports each edit no test notices — a test that runs without deciding. A
# sensor: it reports what survived and fails only when the run itself breaks, not on what it counts.
# With a base, only the Rust code changed since it is mutated: the lines changed from the merge base
# to the working tree. The review runs it with the branch the change leaves from, so that a branch
# no test decides is reported before the merge, not a week after it. With --since a time, the base
# is the last commit before it on HEAD's first-parent line: main as it stood then, never a commit
# of a branch merged later, whose code would otherwise fall before every week's base. The weekly
# workflow mutants.yml runs it with a time a week before, so that each week's code is mutated
# once; a history younger than the time has no such commit, and all of it is mutated. Without a
# base it mutates the whole workspace, some 12,000 mutants and far more hours than a runner's job
# may take; a shard, such as 1/50, cuts it to a share that fits. A change that touches no Rust
# code has nothing to mutate. Files git does not track are not part of the diff: a new file is
# mutated once it is added.
#
# Each process cargo-mutants starts may hold 6 GiB of address space. A mutant that lets a loop
# grow a vector, such as one in the label cache's placement or in a circle's step, filled a
# runner's 16 GB within its 300 seconds, and GitHub shut the runner down; under the limit the
# allocation fails and the mutant's tests fail with it. The build and the tests of the whole
# workspace stay under 4 GiB, 24 test threads included.
#
# Each mutant's tests stop after 300 seconds. cargo-mutants would set five times the tests'
# baseline run, some 1,000 seconds on a runner, and a mutant that turns a parser's step into a
# loop spent all of it. 300 seconds stay above the 205 the whole workspace's tests take there.
#
# Usage: bash scripts/mutants.sh [--root <dir>] [--base <revision> | --since <time>]
#          [shard, e.g. 1/4]
#   e.g. bash scripts/mutants.sh --base origin/main
#        bash scripts/mutants.sh --since 2026-11-13T04:17:00Z 3/16
# The root, this repository unless --root names another, is the workspace mutated. An argument it
# does not read, such as an unknown option or a second shard, and a root that is no directory exit
# 2; a usage error of its own options exits 1.
set -uo pipefail
# The diff is taken in this repository, never in one a caller names through git's variables
# (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/mutants.sh [--root <dir>] [--base <revision> | --since <time>] [shard]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
set -- "${ARGS[@]}"
CARGO_MUTANTS_VERSION="27.1.0"
USAGE="usage: bash $usage"

BASE=""
SINCE=""
rest=()
while (($#)); do
  case "$1" in
    --since)
      [[ -z "$SINCE" ]] || { echo "mutants: more than one --since; $USAGE" >&2; exit 1; }
      [[ -n "${2:-}" ]] || { echo "mutants: --since needs a time; $USAGE" >&2; exit 1; }
      SINCE="$2"
      shift 2
      ;;
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
    *)
      rest+=("$1")
      shift
      ;;
  esac
done
rest_args "${BASH_SOURCE[0]}" "$usage" 1 "${rest[@]}"
SHARD="${rest[0]:-}"

if [[ -n "$BASE" && -n "$SINCE" ]]; then
  echo "mutants: --base or --since, not both; $USAGE" >&2
  exit 1
fi

if [[ -n "$SINCE" ]]; then
  BASE="$(git rev-list -1 --first-parent --before="$SINCE" HEAD)" || exit 1
  if [[ -n "$BASE" ]]; then
    echo "mutants: the Rust code changed since $BASE, the last commit before $SINCE"
  else
    echo "mutants: no commit before $SINCE — the whole workspace is mutated"
  fi
fi
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
ulimit -v $((6 * 1024 * 1024)) || exit 1
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
