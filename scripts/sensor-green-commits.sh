#!/usr/bin/env bash
#
# Sensor: commits between a base and a head on which a fast check fails.
#
# A commit of a branch on which a check fails breaks a bisect through it, though the hooks and CI
# hold only the head green. This sensor runs the fast checks on each commit between a base and a
# head, the head and merge commits included, and lists each commit on which one fails, with the
# check. It reports and never fails; a listed commit that a review saw is not rewritten for it,
# and a bisect skips it. The fast checks are those that build nothing and run no test:
#   - rustfmt: 'cargo fmt --all --check', where the commit has a Cargo.toml at its root;
#   - shell lint: the commit's own scripts/check-shell.sh;
#   - notice: the commit's own scripts/check-notice.sh;
#   - docs: the commit's own scripts/check-docs.sh.
# A check a commit does not have is not run on it. Clippy, the build, and the tests are left out:
# they cost minutes a commit.
#
# Each commit is checked out into a temporary clone that shares the repository's objects, so the
# repository's working tree, refs, and worktrees stay as they are. A check runs on a commit only
# where the commit changes a path the check reads, against its first parent; elsewhere the
# commit keeps its parent's verdict, and a parent outside the range counts as passing, as the base
# passed the gate that brought it to main. The documentation checks read every text file, so they
# run on every commit. The checks run without the variables by which CI names the branch a change
# lands on, so a draft ADR on a commit before its numbering is no finding (check 16 of
# check-docs.sh), and without CI=true, so a ShellCheck of another version warns instead of
# failing. Where cargo is missing, rustfmt is not run, and the report says so.
#
# Pure bash + git, and cargo for rustfmt. Usage:
#     scripts/sensor-green-commits.sh [--root <dir>] [base [head]]
# It reads the repository it lives in, as the checks do; --root names another, as its self-test
# does with each fixture.
# Without a base, the merge base of head with origin/main (else main) is used; CI passes the
# pull request's base. A base of zeros, which a push event reports for a new branch, counts as
# none. With nothing to compare — on main without a remote, or base and head the same — it says
# so. Exit code 0 always, except 2 on a base or head that does not resolve.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it reads (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/git.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/git.sh"

NAME="Green commits sensor"
enter_root "$NAME" "${BASH_SOURCE[0]}" "$@"
shift "$ROOT_TAKEN"

resolve_change "$NAME" "${1:-}" "${2:-HEAD}" || exit 0
base="$CHANGE_BASE" head="$CHANGE_HEAD"
if [[ "$(git rev-parse "$base^{commit}")" == "$(git rev-parse "$head^{commit}")" ]]; then
  echo "$NAME: nothing to compare — base and head are the same commit."
  exit 0
fi

# The checks: the label a finding names, the script a commit runs (empty for rustfmt), and the
# paths whose change can move the verdict, as git pathspecs ('*' matches across '/'); none means
# every commit runs it.
labels=("rustfmt" "shell lint (scripts/check-shell.sh)" "notice (scripts/check-notice.sh)" "docs (scripts/check-docs.sh)")
scripts=("" scripts/check-shell.sh scripts/check-notice.sh scripts/check-docs.sh)
inputs=("*.rs *Cargo.toml *rustfmt.toml rust-toolchain.toml"
        "*.sh .githooks scripts .gitignore .shellcheckrc"
        "crates fuzz THIRD-PARTY-NOTICES.md scripts/check-notice.sh scripts/lib"
        "")

have_cargo=1
command -v cargo >/dev/null 2>&1 || have_cargo=0

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tree="$tmp/tree"
git clone -q --shared --no-checkout . "$tree" 2>/dev/null \
  || { echo "$NAME: could not clone the repository into a temporary directory." >&2; exit 0; }
git -C "$tree" remote remove origin

# run_check <i> — run check i on the commit checked out in the clone; 0 when it passes or the
# commit does not have it.
run_check() {
  local script="${scripts[$1]}"
  if [[ -z "$script" ]]; then
    [[ -f "$tree/Cargo.toml" ]] && ((have_cargo)) || return 0
    (cd "$tree" && cargo fmt --all --check) >/dev/null 2>&1
    return
  fi
  [[ -f "$tree/$script" ]] || return 0
  (cd "$tree" && env -u CI -u GITHUB_ACTIONS -u GITHUB_BASE_REF -u GITHUB_REF_NAME \
      -u PULL_REQUEST_DRAFT bash "$script") >/dev/null 2>&1
}

declare -A failing # "<sha> <i>" of a commit on which check i fails
findings=() count=0 rustfmt_seen=0
while read -r sha; do
  count=$((count + 1))
  parent="$(git rev-parse -q --verify "$sha^1" 2>/dev/null)"
  git -C "$tree" checkout -q --detach "$sha" 2>/dev/null
  failed=() pids=()
  for i in "${!labels[@]}"; do
    pids[i]=""
    # Unchanged inputs keep the parent's verdict.
    if [[ -n "$parent" && -n "${inputs[i]}" ]]; then
      read -r -a specs <<< "${inputs[i]}"
      if git diff --quiet "$parent" "$sha" -- "${specs[@]}"; then
        [[ -n "${failing["$parent $i"]:-}" ]] && failing["$sha $i"]=1
        continue
      fi
    fi
    ((i == 0)) && [[ -f "$tree/Cargo.toml" ]] && rustfmt_seen=1
    # The checks of one commit run side by side; each only reads the clone.
    run_check "$i" &
    pids[i]=$!
  done
  for i in "${!labels[@]}"; do
    [[ -n "${pids[i]}" ]] || continue
    wait "${pids[i]}" || failing["$sha $i"]=1
  done
  for i in "${!labels[@]}"; do
    [[ -n "${failing["$sha $i"]:-}" ]] && failed+=("${labels[i]}")
  done
  ((${#failed[@]})) || continue
  joined="$(printf '%s, ' "${failed[@]}")"
  findings+=("$(git rev-parse --short "$sha") $(git log -1 --format=%s "$sha") — fails: ${joined%, }")
done < <(git rev-list --reverse --topo-order "$base..$head")

short_base="$(git rev-parse --short "$base")"
short_head="$(git rev-parse --short "$head")"
note=""
((rustfmt_seen && !have_cargo)) && note=" rustfmt was not run: cargo is not on PATH."
if ((${#findings[@]} == 0)); then
  echo "$NAME ($short_base..$short_head): nothing to report — each of $count commit(s) passes the fast checks.$note"
  exit 0
fi
echo "$NAME ($short_base..$short_head): ${#findings[@]} commit(s) of $count fail a fast check — a bisect that lands on one stops there; check it out and run the check to see why:$note"
printf '  - %s\n' "${findings[@]}"
exit 0
