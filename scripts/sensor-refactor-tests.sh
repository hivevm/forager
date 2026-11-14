#!/usr/bin/env bash
#
# Sensor: refactor commits that change a test (docs/CONVENTIONS.md — a commit typed 'refactor'
# changes nothing a test can see).
#
# Lists, between a base and a head, each commit typed 'refactor' whose diff adds or removes an
# assertion or a test attribute in a Rust file: a line that calls 'assert!', 'assert_eq!', or
# 'assert_ne!', or carries '#[test]'. Such a commit changed what a test holds, and the convention
# asks for a commit typed 'fix' of its own instead. It reports and never fails. A listed commit
# may still be a refactor, one that moves a test or renames what an assertion calls; the reviewer
# holds each against the convention. It reads only the changed line, so it cannot see a changed
# expected value off the assertion's first line: on a continuation line of an assertion rustfmt
# wrapped, in a 'let expected = …' binding, in a fixture, or in a reference image.
#
# A commit is typed 'refactor' when its subject starts 'refactor:', 'refactor(<scope>):', or
# either with a '!' before the colon. Merge commits are not read.
#
# Pure bash + git + awk. Usage:
#     scripts/sensor-refactor-tests.sh [--root <dir>] [base [head]]
# It reads the repository it lives in, as the checks do; --root names another, as its self-test
# does with each fixture.
# Without a base, the merge base of head with origin/main (else main) is used; CI passes the
# pull request's base. A base of zeros, which a push event reports for a new branch, counts as
# none. With nothing to compare — on main without a remote, or base and head the same — it says
# so. Exit code 0 always, except 2 on a base or head that does not resolve.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/git.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/git.sh"

enter_root "Refactor tests sensor" "${BASH_SOURCE[0]}" "$@"
shift "$ROOT_TAKEN"

resolve_change "Refactor tests sensor" "${1:-}" "${2:-HEAD}" || exit 0
base="$CHANGE_BASE" head="$CHANGE_HEAD"
if [[ "$(git rev-parse "$base^{commit}")" == "$(git rev-parse "$head^{commit}")" ]]; then
  echo "Refactor tests sensor: nothing to compare — base and head are the same commit."
  exit 0
fi

# An added or removed line of a Rust file that asserts or marks a test; 'debug_assert!' is a
# check of the code, not of a test, and is left out by the character before 'assert'.
TEST_LINE='^[-+].*((^|[^A-Za-z0-9_])assert(_eq|_ne)?!|#\[test\])'

findings=()
while IFS=$'\t' read -r sha subject; do
  [[ "$subject" =~ ^refactor(\([^\)]*\))?!?: ]] || continue
  # The paths of the commit's Rust files with a changed line that asserts or marks a test.
  # The prefixes are given, as a user's diff.noprefix or diff.mnemonicPrefix would change them.
  mapfile -t paths < <(git -c core.quotePath=false show --format= -U0 --no-color \
      --src-prefix=a/ --dst-prefix=b/ "$sha" -- '*.rs' \
    | TEST_LINE="$TEST_LINE" awk '
        # A file header comes between "diff" and its first hunk, so a changed line whose content
        # starts "-- " or "++ " is not taken for one. The new path names the file, or the old one
        # for a deleted file; a path with a space ends in a tab, which is no part of it.
        /^diff /                 { hunk = 0; next }
        /^@@/                    { hunk = 1; next }
        !hunk && /^--- /         { file = substr($0, 7); sub(/\t$/, "", file); next }
        !hunk && /^\+\+\+ /     { if ($0 != "+++ /dev/null") { file = substr($0, 7); sub(/\t$/, "", file) } next }
        hunk && $0 ~ ENVIRON["TEST_LINE"] { if (!(file in seen)) { seen[file] = 1; print file } }
      ')
  ((${#paths[@]})) || continue
  findings+=("$(git rev-parse --short "$sha") $subject — changes a test in: ${paths[*]}")
done < <(git log --no-merges --reverse --format='%H%x09%s' "$base..$head")

short_base="$(git rev-parse --short "$base")"
short_head="$(git rev-parse --short "$head")"
if ((${#findings[@]} == 0)); then
  echo "Refactor tests sensor ($short_base..$short_head): nothing to report."
  exit 0
fi
echo "Refactor tests sensor ($short_base..$short_head): ${#findings[@]} commit(s) typed refactor that change a test — a behaviour each moves is a fix of its own, or it needs its reason in the pull request (docs/CONVENTIONS.md):"
printf '  - %s\n' "${findings[@]}"
exit 0
