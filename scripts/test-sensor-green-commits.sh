#!/usr/bin/env bash
#
# Self-test of scripts/sensor-green-commits.sh.
#
# Each case builds a throwaway git repository under a temporary directory with a base commit
# holding a formatted Rust crate, adds commits, runs the sensor against the base, and asserts the
# exit code and the line the reviewer would read. A case either lists a commit on which a fast
# check fails, with the check, or holds the sensor silent on one it must not list. The notice and
# documentation checks of a fixture are stubs the fixture commits, as the sensor runs each
# commit's own scripts. The rustfmt cases need cargo with rustfmt, as the rust job of CI has it.
#
# Verifies: ADR-0002
# Inputs: scripts/sensor-green-commits.sh scripts/lib/
#
# Usage:
#     scripts/test-sensor-green-commits.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn the fixtures' git on its
# repository; they run on their own (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"
SENSOR="$ROOT/scripts/sensor-green-commits.sh"

FORMATTED='pub fn two() -> u32 {\n    2\n}\n'
UNFORMATTED='pub fn two()->u32{2}\n'

# repo <name> — a repository whose base commit, tagged 'base', holds a crate rustfmt passes;
# printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/src"
  git -C "${dir:?}" init -q -b main
  printf '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2021"\n' > "$dir/Cargo.toml"
  # shellcheck disable=SC2059 # the format is the fixture's source
  printf "$FORMATTED" > "$dir/src/lib.rs"
  commit "$dir" "feat: two"
  git -C "${dir:?}" tag base
  echo "$dir"
}

# lib <dir> <source> — write the crate's source as the printf format given.
# shellcheck disable=SC2059 # the format is the fixture's source
lib() { printf "$2" > "$1/src/lib.rs"; }

# stub <dir> <check> <script body> — a check script of the fixture, scripts/<check>.sh, that runs
# the body from the fixture's root.
stub() {
  mkdir -p "$1/scripts"
  # shellcheck disable=SC2016 # the stub expands its own variables
  printf '#!/usr/bin/env bash\ncd "$(dirname "$0")/.." || exit 2\n%s\n' "$3" > "$1/scripts/$2.sh"
}

# expect_sensor <exit code> <name> <dir> <needle> [base] — run the sensor in the repository
# against the given base (default 'base'); the output must contain the needle, and not $REJECT
# when that is set (expect, scripts/lib/expect.sh).
expect_sensor() {
  fixture "$3"
  expect "$2" "$1" "$4" bash "$SENSOR" --root "$3" "${5-base}"
}

# short <dir> [rev] — the abbreviated hash of a commit of the repository, the head by default.
short() { fixture "$1"; git -C "${1:?}" rev-parse --short "${2:-HEAD}"; }

# --- cases -----------------------------------------------------------------------------------

d="$(repo rustfmt-between)"
lib "$d" "$UNFORMATTED"
commit "$d" "feat: two in one line"
bad="$(short "$d")"
lib "$d" "$FORMATTED"
commit "$d" "style: two formatted"
expect_sensor 0 "a commit that fails rustfmt between a green base and a green head is named with the check" \
  "$d" "$bad feat: two in one line — fails: rustfmt"
REJECT="style: two formatted" expect_sensor 0 "the green head that fixes it is not listed" "$d" "1 commit(s)"

d="$(repo carried)"
lib "$d" "$UNFORMATTED"
commit "$d" "feat: two in one line"
printf '# App\n' > "$d/README.md"
commit "$d" "docs: a readme"
readme="$(short "$d")"
lib "$d" "$FORMATTED"
commit "$d" "style: two formatted"
expect_sensor 0 "a commit after the failing one that touches no Rust file still fails rustfmt" \
  "$d" "$readme docs: a readme — fails: rustfmt"
expect_sensor 0 "both failing commits are counted" "$d" "2 commit(s)"

d="$(repo own-scripts)"
stub "$d" check-notice 'if grep -rl NO-NOTICE crates; then exit 1; fi'
commit "$d" "feat: a notice check"
mkdir -p "$d/crates"
printf '// NO-NOTICE\n' > "$d/crates/port.rs"
commit "$d" "feat: a port without its notice"
bad="$(short "$d")"
printf '// NOTICE\n' > "$d/crates/port.rs"
commit "$d" "fix: the port keeps its notice"
expect_sensor 0 "a commit that fails the commit's own notice check is named with it" "$d" \
  "$bad feat: a port without its notice — fails: notice (scripts/check-notice.sh)"
REJECT="fix: the port" expect_sensor 0 "the commit that fixes it is not listed" "$d" "1 commit(s)"

d="$(repo two-checks)"
stub "$d" check-docs 'if [[ -e TODO ]]; then exit 1; fi'
commit "$d" "feat: a docs check"
lib "$d" "$UNFORMATTED"
: > "$d/TODO"
commit "$d" "feat: two at once"
bad="$(short "$d")"
lib "$d" "$FORMATTED"
rm "$d/TODO"
commit "$d" "fix: both"
expect_sensor 0 "a commit that fails two checks names both" "$d" \
  "$bad feat: two at once — fails: rustfmt, docs (scripts/check-docs.sh)"

d="$(repo ci-names-no-branch)"
# shellcheck disable=SC2016 # the stub expands its own variables
stub "$d" check-docs 'if [[ -n "${GITHUB_BASE_REF:-}${GITHUB_ACTIONS:-}" ]]; then exit 1; fi'
commit "$d" "feat: a docs check that reads CI's branch"
GITHUB_BASE_REF=main GITHUB_ACTIONS=true expect_sensor 0 \
  "a check reads no branch CI names, so a draft ADR on a commit before its numbering is no finding" \
  "$d" "nothing to report"

d="$(repo merge)"
git -C "${d:?}" checkout -q -b side
lib "$d" "$UNFORMATTED"
commit "$d" "feat: two in one line on a side branch"
git -C "${d:?}" checkout -q main
printf '# App\n' > "$d/README.md"
commit "$d" "docs: a readme"
git -C "${d:?}" -c user.name=fixture -c user.email=fixture@example.invalid \
  merge -q --no-ff -m "merge side" side >/dev/null 2>&1
expect_sensor 0 "a merge that brings a failing commit fails too" "$d" "$(short "$d") merge side — fails: rustfmt"
REJECT="docs: a readme" expect_sensor 0 "the green commit beside the side branch is not listed" "$d" "2 commit(s)"

# The cache: a stub documentation check writes the commit it judges to a file outside the fixture,
# so each run's judged commits can be counted.
d="$(repo cached)"
marks="$work/cached-marks"
stub "$d" check-docs "git rev-parse HEAD >> '$marks'; if [[ -e TODO ]]; then exit 1; fi"
commit "$d" "feat: a docs check that leaves a mark"
: > "$d/TODO"
commit "$d" "feat: a todo"
bad="$(short "$d")"
judged() { wc -l < "$marks"; }
expect_sensor 0 "a first run judges each commit" "$d" "$bad feat: a todo — fails: docs"
check "a first run runs the check on each of the two commits" [ "$(judged)" -eq 2 ]
expect_sensor 0 "a second run reports the failure again" "$d" "$bad feat: a todo — fails: docs"
check "a second run judges the failing commit again, and not the passing one" [ "$(judged)" -eq 3 ]
check "the commit judged again is the failing one" [ "$(tail -n 1 "$marks")" == "$(git -C "${d:?}" rev-parse HEAD)" ]
rm "$d/TODO"
commit "$d" "fix: no todo"
REJECT="fix: no todo" expect_sensor 0 "a run after one new commit lists the old failure" "$d" "1 commit(s) of 3"
check "a run after one new commit runs the check on it and the failing one" [ "$(judged)" -eq 5 ]
expect_sensor 0 "a run without new commits" "$d" "1 commit(s) of 3"
check "a run without new commits runs the check on the failing one alone" [ "$(judged)" -eq 6 ]
git -C "${d:?}" -c user.name=fixture -c user.email=fixture@example.invalid \
  commit -q --amend -m "fix: no todo, reworded"
expect_sensor 0 "a rewritten commit is judged again" "$d" "1 commit(s) of 3"
check "a rewritten commit runs the check again" [ "$(judged)" -eq 8 ]
mkdir -p "$work/changed-sensor/scripts"
cp -R "$ROOT/scripts/lib" "$work/changed-sensor/scripts/"
{ cat "$SENSOR"; echo "# changed"; } > "$work/changed-sensor/scripts/sensor-green-commits.sh"
bash "$work/changed-sensor/scripts/sensor-green-commits.sh" --root "$d" base >/dev/null 2>&1
check "a changed sensor judges every commit again" [ "$(judged)" -eq 11 ]

d="$(repo numbering)"
stub "$d" check-docs 'if [[ -e TODO ]]; then exit 1; fi'
commit "$d" "feat: a docs check"
: > "$d/TODO"
commit "$d" "docs(adr): number the drafts as ADR-0001"
rm "$d/TODO"
commit "$d" "docs: cite the drafts by their numbers"
expect_sensor 0 "the first numbering commit, which ADR-0002 has fail the docs checks, is no finding" \
  "$d" "nothing to report"
: > "$d/TODO"
commit "$d" "docs(adr): a todo"
expect_sensor 0 "another commit typed docs(adr) is still judged" "$d" "docs(adr): a todo — fails: docs"

d="$(repo shell-lint)"
stub "$d" check-shell 'if [[ -d tools ]] && grep -rl BAD tools; then exit 1; fi'
commit "$d" "feat: a shell lint"
mkdir -p "$d/tools"
printf 'BAD\n' > "$d/tools/run.sh"
commit "$d" "feat: a script the lint refuses"
bad="$(short "$d")"
printf '# App\n' > "$d/README.md"
commit "$d" "docs: a readme"
readme="$(short "$d")"
printf 'true\n' > "$d/tools/run.sh"
commit "$d" "fix: the script passes"
expect_sensor 0 "a commit that fails the commit's own shell lint is named with it" "$d" \
  "$bad feat: a script the lint refuses — fails: shell lint (scripts/check-shell.sh)"
REJECT="fix: the script passes" expect_sensor 0 \
  "a commit that touches no shell script keeps the failing verdict, and the fix is not listed" "$d" \
  "$readme docs: a readme — fails: shell lint (scripts/check-shell.sh)"

d="$(repo green)"
printf '# App\n' > "$d/README.md"
commit "$d" "docs: a readme"
expect_sensor 0 "a branch whose every commit passes" "$d" "nothing to report"

d="$(repo untouched)"
lib "$d" "$UNFORMATTED"
commit "$d" "feat: two in one line"
printf 'scratch\n' > "$d/untracked.txt"
before="$(git -C "${d:?}" for-each-ref; git -C "${d:?}" worktree list --porcelain; git -C "${d:?}" status --porcelain; git -C "${d:?}" rev-parse HEAD)"
bash "$SENSOR" --root "$d" base >/dev/null 2>&1
after="$(git -C "${d:?}" for-each-ref; git -C "${d:?}" worktree list --porcelain; git -C "${d:?}" status --porcelain; git -C "${d:?}" rev-parse HEAD)"
check "the repository's refs, worktrees, head, and working tree stay as they were" [ "$before" == "$after" ]

d="$(repo same-commit)"
expect_sensor 0 "base and head the same commit" "$d" "base and head are the same commit"

d="$(repo bad-base)"
expect_sensor 2 "a base that does not resolve" "$d" "does not resolve" nosuchref

d="$(repo no-main)"
git -C "${d:?}" branch -m main work
expect_sensor 0 "no base and no main" "$d" "no main to compare with" ""

# --- summary ---------------------------------------------------------------------------------

summary "Green commits sensor self-test"
