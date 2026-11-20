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
#   - comment width: the commit's own scripts/check-comment-width.sh;
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
# lands on, so that a check judges a commit by the commit alone, and without CI=true, so a
# ShellCheck of another version warns instead of failing. Where cargo is missing, rustfmt is not
# run, and where ShellCheck is, each commit's shell lint skips it and passes; the report's first
# line says so of either.
#
# Each passing verdict, by commit and check, is kept in 'green-commits-verdicts' of the repository's
# git directory, shared by its worktrees, as the pre-push hook keeps its green trees. A later run
# judges only the commits it has no passing verdict for: a branch pushed again costs its new
# commits and those that failed. A failure is judged again on each run, since the machine may have
# caused it, such as a toolchain rustup could not fetch or a check killed under load. A
# commit's hash names its content and its own check scripts, so a rewritten commit is judged
# again. The verdicts stand under a key of this script, scripts/lib/git.sh, and the versions of
# rustfmt and ShellCheck found on PATH; a change of any judges every commit again. CI starts
# without the file and judges every commit.
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
SELF_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SELF="$SELF_DIR/$(basename "${BASH_SOURCE[0]}")"
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
labels=("rustfmt" "shell lint (scripts/check-shell.sh)" "notice (scripts/check-notice.sh)"
        "comment width (scripts/check-comment-width.sh)" "docs (scripts/check-docs.sh)")
scripts=("" scripts/check-shell.sh scripts/check-notice.sh scripts/check-comment-width.sh
         scripts/check-docs.sh)
inputs=("*.rs *Cargo.toml *rustfmt.toml rust-toolchain.toml"
        "*.sh .githooks scripts .gitignore .shellcheckrc"
        "crates fuzz THIRD-PARTY-NOTICES.md scripts/check-notice.sh scripts/lib"
        "crates/*.rs fuzz/*.rs scripts/check-comment-width.sh"
        "")

have_cargo=1 have_shellcheck=1
command -v cargo >/dev/null 2>&1 || have_cargo=0
command -v shellcheck >/dev/null 2>&1 || have_shellcheck=0

# The verdicts of earlier runs, by commit and check, under a key of the sensor's own code and the
# versions of rustfmt and ShellCheck: a change of either judges every commit again.
cache="$(cd "$(git rev-parse --git-common-dir)" && pwd)/green-commits-verdicts"
tool_key="$(
  { cat "$SELF" "$SELF_DIR/lib/git.sh"
    ((have_cargo)) && cargo fmt --version 2>&1
    ((have_shellcheck)) && shellcheck --version 2>&1
  } | git hash-object --stdin
)"
declare -A cached # "<sha> <i>" → pass or fail, from an earlier run
if [[ -f "$cache" ]]; then
  while read -r key sha i verdict; do
    [[ "$key" == "$tool_key" ]] && cached["$sha $i"]="$verdict"
  done < "$cache"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
tree="$tmp/tree"
cloned=0

# checkout <sha> — check the commit out into the temporary clone, made at the first call.
checkout() {
  if ((!cloned)); then
    git clone -q --shared --no-checkout . "$tree" 2>/dev/null || return 1
    git -C "$tree" remote remove origin
    cloned=1
  fi
  git -C "$tree" checkout -q --detach "$1" 2>/dev/null
}

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

# verdict["<sha> <i>"] is pass or fail; known["<sha> <i>"] is set where it was judged, here or by
# an earlier run, and not taken from a parent outside the range that counts as passing. Only a
# known pass is kept for the next run.
declare -A verdict known
new_entries=()
findings=() count=0 rustfmt_seen=0 shell_seen=0
while read -r sha; do
  count=$((count + 1))
  parent="$(git rev-parse -q --verify "$sha^1" 2>/dev/null)"
  failed=() pids=() checked_out=0 unchecked=0
  for i in "${!labels[@]}"; do
    pids[i]=""
    if [[ -n "${cached["$sha $i"]:-}" ]]; then
      verdict["$sha $i"]="${cached["$sha $i"]}" known["$sha $i"]=1
      continue
    fi
    # Unchanged inputs keep the parent's verdict.
    if [[ -n "$parent" && -n "${inputs[i]}" ]]; then
      read -r -a specs <<< "${inputs[i]}"
      if git diff --quiet "$parent" "$sha" -- "${specs[@]}"; then
        if [[ -n "${verdict["$parent $i"]:-}" ]]; then
          verdict["$sha $i"]="${verdict["$parent $i"]}"
          [[ -n "${known["$parent $i"]:-}" ]] && known["$sha $i"]=1
        elif [[ -n "${cached["$parent $i"]:-}" ]]; then
          verdict["$sha $i"]="${cached["$parent $i"]}" known["$sha $i"]=1
        else
          verdict["$sha $i"]=pass
        fi
        [[ -n "${known["$sha $i"]:-}" && "${verdict["$sha $i"]}" == pass ]] \
          && new_entries+=("$tool_key $sha $i pass")
        continue
      fi
    fi
    if ((!checked_out)); then
      if ! checkout "$sha"; then
        unchecked=1
        break
      fi
      checked_out=1
    fi
    ((i == 0)) && [[ -f "$tree/Cargo.toml" ]] && rustfmt_seen=1
    [[ "${scripts[i]}" == scripts/check-shell.sh && -f "$tree/${scripts[i]}" ]] && shell_seen=1
    # The checks of one commit run side by side; each only reads the clone.
    run_check "$i" &
    pids[i]=$!
  done
  for i in "${!labels[@]}"; do
    [[ -n "${pids[i]:-}" ]] || continue
    if wait "${pids[i]}"; then verdict["$sha $i"]=pass; else verdict["$sha $i"]=fail; fi
    known["$sha $i"]=1
    [[ "${verdict["$sha $i"]}" == pass ]] && new_entries+=("$tool_key $sha $i pass")
  done
  subject="$(git rev-parse --short "$sha") $(git log -1 --format=%s "$sha")"
  if ((unchecked)); then
    # Its children judge on their own, as after a parent outside the range.
    for i in "${!labels[@]}"; do unset 'verdict["$sha $i"]' 'known["$sha $i"]'; done
    findings+=("$subject — not checked: git could not check it out into a temporary clone")
    continue
  fi
  for i in "${!labels[@]}"; do
    [[ "${verdict["$sha $i"]:-}" == fail ]] && failed+=("${labels[i]}")
  done
  ((${#failed[@]})) || continue
  joined="$(printf '%s, ' "${failed[@]}")"
  findings+=("$subject — fails: ${joined%, }")
done < <(git rev-list --reverse --topo-order "$base..$head")

# The cache keeps its last 20000 verdicts, about 5000 commits; a run beside another may lose the
# other's, which the next run judges again.
if ((${#new_entries[@]})); then
  { tail -n 20000 "$cache" 2>/dev/null; printf '%s\n' "${new_entries[@]}"; } \
    | tail -n 20000 > "$cache.$$" && mv "$cache.$$" "$cache"
fi

short_base="$(git rev-parse --short "$base")"
short_head="$(git rev-parse --short "$head")"
note=""
((rustfmt_seen && !have_cargo)) && note=" rustfmt was not run: cargo is not on PATH."
((shell_seen && !have_shellcheck)) && note+=" The shell lint ran without ShellCheck, which is not on PATH, so it passed what ShellCheck alone refuses."
if ((${#findings[@]} == 0)); then
  echo "$NAME ($short_base..$short_head): nothing to report — each of $count commit(s) passes the fast checks.$note"
  exit 0
fi
echo "$NAME ($short_base..$short_head): ${#findings[@]} commit(s) of $count fail a fast check — a bisect that lands on one stops there; check it out and run the check to see why:$note"
printf '  - %s\n' "${findings[@]}"
exit 0
