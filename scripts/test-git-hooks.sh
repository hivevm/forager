#!/usr/bin/env bash
#
# Self-test of the git hooks under .githooks/: pre-commit refuses a commit on main, pre-push
# refuses a push while the checks are red (AGENTS.md, sections 5 and 6).
#
# Each case builds a throwaway repository under a temporary directory with core.hooksPath pointing
# at this repository's .githooks/, runs a real 'git commit' or 'git push' in it — the push goes to
# a bare repository next to it — and asserts the exit code and, for a refused command, the message.
# A push case ships its own scripts/check-all.sh, green or red, so the test decides what the hook
# finds.
#
# Usage:
#     scripts/test-git-hooks.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn the fixtures' git on its
# repository; they run on their own (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS="$ROOT/.githooks"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# g <dir> <git args...> — git in the fixture with an identity, so that commits need no config.
g() {
  local dir="$1"; shift
  git -C "${dir:?}" -c user.name=t -c user.email=t@example.invalid "$@"
}

# repo <name> — a fixture repository with one commit on main, the hooks enabled after that
# commit, and a bare remote 'origin' to push to. Prints its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir"
  git -C "${dir:?}" init -q -b main
  g "$dir" commit -q --allow-empty -m init
  git -C "${dir:?}" config core.hooksPath "$HOOKS"
  git init -q --bare "$dir.git"
  git -C "${dir:?}" remote add origin "$dir.git"
  echo "$dir"
}

# checks <dir> <exit code> — give the fixture a scripts/check-all.sh that exits as told.
checks() {
  mkdir -p "$1/scripts"
  printf '#!/usr/bin/env bash\necho "check output for the test"\nexit %s\n' "$2" > "$1/scripts/check-all.sh"
}

# expect <pass|refuse> <name> <dir> <needle> <git args...> — run the git command; a refused one
# must exit non-zero with the needle in its output, a passing one exit zero.
expect() {
  local want="$1" name="$2" dir="$3" needle="$4"
  shift 4
  local out rc
  out="$(g "$dir" "$@" 2>&1)"; rc=$?
  if [[ "$want" == "pass" && $rc -eq 0 ]] \
     || [[ "$want" == "refuse" && $rc -ne 0 && "$out" == *"$needle"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected $want${needle:+ with \"$needle\"}, exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

# --- cases ---------------------------------------------------------------------------------

d="$(repo on-main)"
expect refuse "a commit on main" "$d" "on 'main'" commit -q --allow-empty -m x
expect pass "a push from main with no check script to run" "$d" "" push -q origin main

d="$(repo on-branch)"
git -C "${d:?}" switch -q -c feature
expect pass "a commit on a branch" "$d" "" commit -q --allow-empty -m x

d="$(repo red)"
git -C "${d:?}" switch -q -c feature
checks "$d" 1
expect refuse "a push while the checks are red" "$d" "the checks are red" push -q origin feature
expect refuse "the refusal shows the check output" "$d" "check output for the test" push -q origin feature
expect pass "a commit is not gated by the checks" "$d" "" commit -q --allow-empty -m x

d="$(repo green)"
git -C "${d:?}" switch -q -c feature
checks "$d" 0
expect pass "a push while the checks are green" "$d" "" push -q origin feature

d="$(repo detached)"
git -C "${d:?}" checkout -q --detach
expect pass "a commit on a detached HEAD is not on main" "$d" "" commit -q --allow-empty -m x

d="$(repo worktree)"
git -C "${d:?}" worktree add -q -b feature "$d-wt"
# A check that finds its repository from the working directory, then runs git in a repository of
# its own — config, a branch, a push — as the self-tests and the corpus fetch do: with the hook's
# repository variables, all of it would land in the fixture instead.
mkdir -p "$d-wt/scripts"
cat > "$d-wt/scripts/check-all.sh" <<'EOF'
#!/usr/bin/env bash
set -e
[[ "$(git rev-parse --show-toplevel)" == "$PWD" ]]
own="$(mktemp -d)"
git -C "$own" init -q -b main
git -C "$own" config fixture.touched yes
git -C "$own" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m own
git -C "$own" branch leaked
git init -q --bare "$own.git"
git -C "$own" push -q "$own.git" leaked
EOF
refs() { git -C "$d" for-each-ref; git -C "$d.git" for-each-ref; }
before="$(refs | grep -v '/feature$')"
TMPDIR="$work" expect pass "a push from a linked worktree" "$d-wt" "" push -q origin feature
if [[ -z "$(git -C "${d:?}" config --get fixture.touched)" ]] \
   && [[ "$(refs | grep -v '/feature$')" == "$before" ]]; then
  passed=$((passed + 1))
else
  failed=$((failed + 1))
  echo "FAIL: the checks of a push from a linked worktree changed the repository's config or refs" >&2
fi

# --- summary -------------------------------------------------------------------------------

if ((failed)); then
  echo "git hooks self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "git hooks self-test passed ($passed cases)."
exit 0
