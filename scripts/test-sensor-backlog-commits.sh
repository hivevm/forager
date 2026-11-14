#!/usr/bin/env bash
#
# Self-test of scripts/sensor-backlog-commits.sh.
#
# Each case builds a throwaway git repository under a temporary directory with a base commit
# holding a source file and one backlog entry, adds one or more commits, runs the sensor against
# the base, and asserts the exit code and the line the reviewer would read. A case either lists a
# commit that adds a backlog entry beside other changes, or holds the sensor silent on one it
# must not list.
#
# Verifies: ADR-0005
# Inputs: scripts/sensor-backlog-commits.sh scripts/lib/
#
# Usage:
#     scripts/test-sensor-backlog-commits.sh
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
SENSOR="$ROOT/scripts/sensor-backlog-commits.sh"

# repo <name> — a repository whose base commit, tagged 'base', holds src/a.rs and the backlog
# entry docs/backlog/old.md; printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/src" "$dir/docs/backlog"
  git -C "${dir:?}" init -q -b main
  printf 'pub fn a() {}\n' > "$dir/src/a.rs"
  printf '# Old\n' > "$dir/docs/backlog/old.md"
  commit "$dir" "feat: a"
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

# lacks <name> <dir> <needle> — run the sensor against 'base'; the output must not contain the
# needle.
lacks() {
  local name="$1" dir="$2" needle="$3" out
  fixture "$dir"
  out="$(bash "$SENSOR" --root "$dir" base 2>&1)"
  check "$name" test "${out#*"$needle"}" == "$out"
}

# short <dir> — the abbreviated hash of the repository's head.
short() { fixture "$1"; git -C "${1:?}" rev-parse --short HEAD; }

# --- cases -----------------------------------------------------------------------------------

d="$(repo entry-beside-code)"
printf '# X\n' > "$d/docs/backlog/x.md"
printf 'pub fn a() { let _ = 1; }\n' > "$d/src/a.rs"
commit "$d" "feat: a and an entry"
expect_sensor 0 "a commit that adds an entry and changes code is listed, with what lies beside it" "$d" \
  "$(short "$d") feat: a and an entry — adds docs/backlog/x.md beside: src/a.rs"

d="$(repo entry-beside-new-file)"
printf '# X\n' > "$d/docs/backlog/x.md"
printf 'fn b() {}\n' > "$d/src/b.rs"
commit "$d" "feat: b and an entry"
expect_sensor 0 "a file added outside the backlog is a change beside the entry" "$d" \
  "adds docs/backlog/x.md beside: src/b.rs"

d="$(repo entry-beside-deleted-file)"
printf '# X\n' > "$d/docs/backlog/x.md"
git -C "${d:?}" rm -q src/a.rs
commit "$d" "chore: drop a, an entry"
expect_sensor 0 "a file deleted outside the backlog is a change beside the entry" "$d" \
  "adds docs/backlog/x.md beside: src/a.rs"

# A stray 'git rm' of a file renamed into the backlog: without rename detection the commit adds
# the entry and deletes a path outside it.
d="$(repo renamed-into-backlog)"
git -C "${d:?}" mv src/a.rs docs/backlog/a.md
commit "$d" "docs(backlog): a"
expect_sensor 0 "a file moved into the backlog is an entry added beside a deletion" "$d" \
  "adds docs/backlog/a.md beside: src/a.rs"

d="$(repo entry-alone)"
printf '# X\n' > "$d/docs/backlog/x.md"
printf '# Y\n' > "$d/docs/backlog/y.md"
commit "$d" "docs(backlog): x and y"
expect_sensor 0 "a commit that adds only entries lists none" "$d" "nothing to report"

d="$(repo ticket-turned-into-entry)"
mkdir -p "$d/docs/plan/spec"
printf '# Ticket\n' > "$d/docs/plan/spec/01-ticket.md"
commit "$d" "docs(plan): a ticket"
git -C "${d:?}" tag ticket
printf '# X\n' > "$d/docs/backlog/x.md"
git -C "${d:?}" rm -q docs/plan/spec/01-ticket.md
commit "$d" "docs(backlog): the ticket stopped, its finding waits"
expect_sensor 0 "a commit that adds an entry and deletes only a ticket lists none" "$d" "nothing to report" ticket

d="$(repo ticket-and-code-beside-entry)"
mkdir -p "$d/docs/plan/spec"
printf '# Ticket\n' > "$d/docs/plan/spec/01-ticket.md"
commit "$d" "docs(plan): a ticket"
git -C "${d:?}" tag ticket
printf '# X\n' > "$d/docs/backlog/x.md"
printf 'pub fn a() { let _ = 7; }\n' > "$d/src/a.rs"
git -C "${d:?}" rm -q docs/plan/spec/01-ticket.md
commit "$d" "feat: a, the ticket, an entry"
expect_sensor 0 "a ticket deleted beside code still lists the code, and not the ticket" "$d" \
  "adds docs/backlog/x.md beside: src/a.rs" ticket

d="$(repo entry-deleted-beside-work)"
printf 'pub fn a() { let _ = 2; }\n' > "$d/src/a.rs"
git -C "${d:?}" rm -q docs/backlog/old.md
commit "$d" "feat: a, the entry's ticket"
expect_sensor 0 "a commit that deletes an entry beside its work lists none" "$d" "nothing to report"

d="$(repo code-alone)"
printf 'pub fn a() { let _ = 3; }\n' > "$d/src/a.rs"
commit "$d" "feat: a"
expect_sensor 0 "a commit that touches no entry lists none" "$d" "nothing to report"

d="$(repo backlog-word)"
mkdir -p "$d/docs/backlogs" "$d/notes/docs/backlog"
printf '# X\n' > "$d/docs/backlogs/x.md"
printf '# X\n' > "$d/notes/docs/backlog/x.md"
printf 'pub fn a() { let _ = 4; }\n' > "$d/src/a.rs"
commit "$d" "feat: a beside look-alikes"
expect_sensor 0 "a path that only looks like the backlog adds no entry" "$d" "nothing to report"

d="$(repo two-of-three)"
printf '# X\n' > "$d/docs/backlog/x.md"
printf 'pub fn a() { let _ = 5; }\n' > "$d/src/a.rs"
commit "$d" "feat: first"
printf '# Y\n' > "$d/docs/backlog/y.md"
commit "$d" "docs(backlog): second"
printf '# Z\n' > "$d/docs/backlog/z.md"
printf 'notes\n' > "$d/README.md"
commit "$d" "docs: third"
expect_sensor 0 "only the commits that add an entry beside other changes are counted" "$d" "2 commit(s)"
expect_sensor 0 "the first is listed" "$d" "feat: first — adds docs/backlog/x.md beside: src/a.rs"
expect_sensor 0 "the third is listed" "$d" "docs: third — adds docs/backlog/z.md beside: README.md"
lacks "the second, an entry alone, is not listed" "$d" "docs(backlog): second"

# A merge whose diff to its first parent adds entries beside a change of its own is not read:
# merges are left out, and their branches' commits are read on their own. Its side adds an entry
# alone, so only the merge could be listed.
d="$(repo merge)"
git -C "${d:?}" checkout -q -b side
printf '# X\n' > "$d/docs/backlog/x.md"
commit "$d" "docs(backlog): x"
git -C "${d:?}" checkout -q main
printf 'pub fn a() { let _ = 6; }\n' > "$d/src/a.rs"
commit "$d" "feat: a"
git -C "${d:?}" -c user.name=fixture -c user.email=fixture@example.invalid \
  merge -q --no-ff --no-commit side >/dev/null 2>&1
printf '# W\n' > "$d/docs/backlog/w.md"
printf 'notes\n' > "$d/README.md"
commit "$d" "Merge side"
expect_sensor 0 "a merge commit is not read" "$d" "nothing to report"

d="$(repo same-commit)"
expect_sensor 0 "base and head the same commit" "$d" "base and head are the same commit"

d="$(repo bad-base)"
expect_sensor 2 "a base that does not resolve" "$d" "does not resolve" nosuchref

d="$(repo no-main)"
git -C "${d:?}" branch -m main work
expect_sensor 0 "no base and no main" "$d" "no main to compare with" ""

# --- summary ---------------------------------------------------------------------------------

summary "Backlog commits sensor self-test"
