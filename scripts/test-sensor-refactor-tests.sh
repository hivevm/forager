#!/usr/bin/env bash
#
# Self-test of scripts/sensor-refactor-tests.sh.
#
# Each case builds a throwaway git repository under a temporary directory with a base commit
# holding a Rust source file and its test, adds one or more commits, runs the sensor against the
# base, and asserts the exit code and the line the reviewer would read. A case either lists one
# commit typed refactor that changes a test, or holds the sensor silent on one it must not list.
#
# Verifies: ADR-0002
# Inputs: scripts/sensor-refactor-tests.sh scripts/lib/
#
# Usage:
#     scripts/test-sensor-refactor-tests.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"
SENSOR="$ROOT/scripts/sensor-refactor-tests.sh"

# repo <name> — a repository whose base commit is tagged 'base', printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/src" "$dir/tests"
  git -C "${dir:?}" init -q -b main
  printf 'pub fn two() -> u32 {\n    debug_assert!(true);\n    2\n}\n' > "$dir/src/lib.rs"
  printf '#[test]\nfn two() {\n    assert_eq!(app::two(), 2);\n}\n' > "$dir/tests/two.rs"
  commit "$dir" "feat: two"
  git -C "${dir:?}" tag base
  echo "$dir"
}

# expect_sensor <exit code> <name> <dir> <needle> [base] — run the sensor in the repository
# against the given base (default 'base'); the output must contain the needle (expect,
# scripts/lib/expect.sh).
expect_sensor() {
  fixture "$3"
  expect "$2" "$1" "$4" bash "$SENSOR" --root "$3" "${5-base}"
}

# short <dir> — the abbreviated hash of the repository's head.
short() { fixture "$1"; git -C "${1:?}" rev-parse --short HEAD; }

# edit <file> <sed script> — rewrite the file through sed in place, keeping its mode, as macOS
# sed has no in-place flag that GNU sed reads alike.
edit() { sed "$2" "$1" > "$1.tmp" && cat "$1.tmp" > "$1" && rm "$1.tmp"; }

# --- cases -----------------------------------------------------------------------------------

d="$(repo assert-changed)"
edit "$d/tests/two.rs" 's/assert_eq!(app::two(), 2)/assert_eq!(app::two(), 3)/'
printf 'pub fn two() -> u32 {\n    3\n}\n' > "$d/src/lib.rs"
commit "$d" "refactor(app): two is three"
expect_sensor 0 "a refactor that changes an assert is listed, with its test file and not the source" "$d" \
  "$(short "$d") refactor(app): two is three — changes a test in: tests/two.rs"

d="$(repo no-prefix)"
git -C "${d:?}" config diff.noprefix true
edit "$d/tests/two.rs" 's/assert_eq!(app::two(), 2)/assert_eq!(app::two(), 4)/'
commit "$d" "refactor(app): two is four"
expect_sensor 0 "a repository of diff.noprefix still lists the test file by its whole path" "$d" \
  "$(short "$d") refactor(app): two is four — changes a test in: tests/two.rs"

d="$(repo test-added)"
printf '\n#[test]\nfn more() {}\n' >> "$d/tests/two.rs"
commit "$d" "refactor!: a test more"
expect_sensor 0 "a refactor marked breaking that adds a #[test]" "$d" "refactor!: a test more — changes a test in: tests/two.rs"

d="$(repo test-deleted)"
git -C "${d:?}" rm -q tests/two.rs
commit "$d" "refactor: drop the test"
expect_sensor 0 "a refactor that deletes a test file is listed by its old path" "$d" "refactor: drop the test — changes a test in: tests/two.rs"

d="$(repo assert-ne-added)"
printf '\nfn more() {\n    assert_ne!(app::two(), 3);\n}\n' >> "$d/tests/two.rs"
commit "$d" "refactor: an assert_ne"
expect_sensor 0 "a refactor that adds an assert_ne! alone is listed" "$d" "refactor: an assert_ne — changes a test in: tests/two.rs"

# In a hunk, a removed line whose content starts "-- " reads "--- ", and an added one that starts
# "++ " reads "+++ ", as a file header does; neither names the file.
d="$(repo dashes-in-a-hunk)"
printf '%s\n' '-- old' > "$d/tests/a.rs"
commit "$d" "chore: a line of dashes"
git -C "${d:?}" tag dashes
printf '%s\n' '++ b/tests/c.rs' '#[test]' > "$d/tests/a.rs"
commit "$d" "refactor: lines that look like file headers"
expect_sensor 0 "a changed line that looks like a file header names no file" "$d" \
  "refactor: lines that look like file headers — changes a test in: tests/a.rs" dashes

d="$(repo source-only)"
printf 'pub fn two() -> u32 {\n    debug_assert!(1 + 1 == 2);\n    1 + 1\n}\n' > "$d/src/lib.rs"
commit "$d" "refactor(app): compute two"
expect_sensor 0 "a refactor that changes no test, a debug_assert aside, lists none" "$d" "nothing to report"

d="$(repo fix-changes-test)"
edit "$d/tests/two.rs" 's/2);/3);/'
commit "$d" "fix(app): two is three"
expect_sensor 0 "a commit typed fix that changes a test is no finding" "$d" "nothing to report"

d="$(repo not-rust)"
printf 'assert_eq!(a, b)\n#[test]\n' > "$d/notes.md"
commit "$d" "refactor: notes on tests"
expect_sensor 0 "an assert in a file that is no Rust is not read" "$d" "nothing to report"

d="$(repo refactoring-word)"
edit "$d/tests/two.rs" 's/2);/3);/'
commit "$d" "refactoring: not the type"
expect_sensor 0 "a subject that only starts with the word is no refactor" "$d" "nothing to report"

d="$(repo two-of-three)"
edit "$d/tests/two.rs" 's/2);/3);/'
commit "$d" "refactor(app): first"
printf '// note\n' >> "$d/src/lib.rs"
commit "$d" "refactor(app): second"
printf '#[test]\nfn third() { assert!(true); }\n' > "$d/tests/third.rs"
commit "$d" "refactor(app): third"
expect_sensor 0 "only the refactor commits that change a test are counted" "$d" "2 commit(s)"
expect_sensor 0 "the first is listed" "$d" "refactor(app): first — changes a test in: tests/two.rs"
expect_sensor 0 "the third is listed" "$d" "refactor(app): third — changes a test in: tests/third.rs"
# The whole clause from the count to the colon, so no word the convention does not hold, such as
# a regression test, fits in it.
expect_sensor 0 "the message says what the convention says, and no more" "$d" \
  "2 commit(s) typed refactor that change a test — a behaviour each moves is a fix of its own, or it needs its reason in the pull request (docs/CONVENTIONS.md):"

# A merge typed refactor that changes an assertion of its own, beyond both its parents, is not
# read: merges are left out, and their branches' commits are read on their own.
d="$(repo merge)"
git -C "${d:?}" checkout -q -b side
printf '// note\n' >> "$d/src/lib.rs"
commit "$d" "chore: note"
git -C "${d:?}" checkout -q main
printf '// other\n' >> "$d/tests/two.rs"
commit "$d" "chore: other note"
git -C "${d:?}" -c user.name=fixture -c user.email=fixture@example.invalid \
  merge -q --no-ff --no-commit side >/dev/null 2>&1
edit "$d/tests/two.rs" 's/2);/3);/'
commit "$d" "refactor: merge side"
expect_sensor 0 "a merge commit is not read" "$d" "nothing to report"

d="$(repo same-commit)"
expect_sensor 0 "base and head the same commit" "$d" "base and head are the same commit"

d="$(repo bad-base)"
expect_sensor 2 "a base that does not resolve" "$d" "does not resolve" nosuchref

d="$(repo no-main)"
git -C "${d:?}" branch -m main work
expect_sensor 0 "no base and no main" "$d" "no main to compare with" ""

# --- summary ---------------------------------------------------------------------------------

summary "Refactor tests sensor self-test"
