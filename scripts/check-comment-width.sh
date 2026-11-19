#!/usr/bin/env bash
#
# Check: a comment line of a Rust file is at most 100 characters wide.
#
# rustfmt wraps code at 100 columns but not comments: its wrap_comments and comment_width are
# unstable, and the pinned toolchain is stable (ADR-0007). This check fails on each line of a .rs
# file under crates/ or fuzz/ that git does not ignore where:
#
#   - the line is a comment: its first characters after the indentation are '//', which takes in
#     '///' and '//!';
#   - the line, its indentation included, is longer than 100 characters.
#
# Characters are counted, not bytes, as a comment may hold '×' or 'π'. A line that starts with
# code is not read, so a string literal, a raw string of a fixture, and a comment after code on its
# line are beyond this check's reach; so is a block comment. A line of a raw string that starts
# with '//' would be taken for a comment.
#
# Pure bash + git. Usage:
#     scripts/check-comment-width.sh [repository root]
# The root defaults to this repository; scripts/test-check-comment-width.sh passes fixtures.
# Exit code 0 when every comment line fits, 1 otherwise, 2 on a root that is no directory.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Comment width check: '${1:-}' is not a directory." >&2; exit 2; }
WIDTH=100
export LC_ALL=C.UTF-8

# The Rust files, collected first, so that a failing git fails the check instead of listing none.
if ! files="$(git -C "$ROOT" ls-files --cached --others --exclude-standard -- \
  'crates/*.rs' 'fuzz/*.rs' 2>/dev/null)"; then
  echo "ERROR: cannot list the Rust files under $ROOT with git" >&2
  echo "Comment width check FAILED." >&2
  exit 1
fi

failures=0
count=0
while IFS= read -r file; do
  [[ -n "$file" && -f "$ROOT/$file" ]] || continue
  count=$((count + 1))
  n=0
  while IFS= read -r line || [[ -n "$line" ]]; do
    n=$((n + 1))
    [[ "$line" =~ ^[[:space:]]*// ]] || continue
    ((${#line} > WIDTH)) || continue
    echo "ERROR: $file:$n: ${#line} characters — wrap the comment at $WIDTH" >&2
    failures=$((failures + 1))
  done < "$ROOT/$file"
done <<< "$files"

noun=files
((count == 1)) && noun="file"
if ((failures)); then
  echo "Comment width check FAILED ($failures comment lines over $WIDTH characters in $count" \
    "$noun)." >&2
  exit 1
fi
echo "Comment width check passed ($count $noun)."
