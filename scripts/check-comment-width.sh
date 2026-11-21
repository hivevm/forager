#!/usr/bin/env bash
#
# Check: a comment line of a Rust file, a shell script, or a workflow is at most 100 characters
# wide.
#
# rustfmt wraps code at 100 columns but not comments: its wrap_comments and comment_width are
# unstable, and the pinned toolchain is stable (OPS-pinned-stable-rust-toolchain). No formatter
# wraps the comments of the scripts and the workflows either. This check fails on each line that
# git does not ignore, of a file it reads, where:
#
#   - the line is a comment: its first characters after the indentation are '//' in a .rs file
#     under crates/ or fuzz/, which takes in '///' and '//!', and '#' in a script
#     scripts/**/*.sh, .agents/skills/*/*.sh, or .githooks/*, or a workflow
#     .github/workflows/*.yml;
#   - the line, its indentation included, is longer than 100 characters.
#
# A script's '# Inputs:' line passes at any width: scripts/check-all.sh reads only the first line
# that starts with it, so its paths cannot be wrapped onto a second line.
#
# Characters are counted, not bytes, as a comment may hold '×' or 'π'. A line that starts with
# code is not read, so a string literal, a raw string of a fixture, and a comment after code on its
# line are beyond this check's reach; so is a block comment. A line of a raw string or of a
# here-document that starts with the comment's characters would be taken for a comment.
#
# Pure bash + git. Usage:
#     scripts/check-comment-width.sh [--root <dir>]
# The root defaults to this repository; scripts/test-check-comment-width.sh passes fixtures.
# Exit code 0 when every comment line fits, 1 otherwise, 2 on an argument it does not read
# or a root that is no directory.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/check-comment-width.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"
WIDTH=100
export LC_ALL=C.UTF-8

# The files, collected first, so that a failing git fails the check instead of listing none.
# Separated by NUL, so that git writes a name of non-ASCII characters as it is, not quoted. A
# pathspec's '*' matches across '/', so 'scripts/*.sh' takes in scripts/lib/.
mapfile -t -d '' files < <(git -C "$ROOT" ls-files -z --cached --others --exclude-standard -- \
  'crates/*.rs' 'fuzz/*.rs' 'scripts/*.sh' '.agents/skills/*/*.sh' '.githooks/*' \
  '.github/workflows/*.yml' 2>/dev/null)
if ! wait "$!"; then
  echo "ERROR: cannot list the files under $ROOT with git" >&2
  echo "Comment width check FAILED." >&2
  exit 1
fi

failures=0
count=0
for file in "${files[@]}"; do
  [[ -f "$ROOT/$file" ]] || continue
  count=$((count + 1))
  comment='^[[:space:]]*#' inputs='^# Inputs:'
  [[ "$file" == *.rs ]] && comment='^[[:space:]]*//' inputs='^$'
  n=0
  while IFS= read -r line || [[ -n "$line" ]]; do
    n=$((n + 1))
    [[ "$line" =~ $comment ]] || continue
    [[ "$line" =~ $inputs ]] && continue
    ((${#line} > WIDTH)) || continue
    echo "ERROR: $file:$n: ${#line} characters — wrap the comment at $WIDTH" >&2
    failures=$((failures + 1))
  done < "$ROOT/$file"
done

noun=files
((count == 1)) && noun="file"
if ((failures)); then
  echo "Comment width check FAILED ($failures comment lines over $WIDTH characters in $count" \
    "$noun)." >&2
  exit 1
fi
echo "Comment width check passed ($count $noun)."
