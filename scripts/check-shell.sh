#!/usr/bin/env bash
#
# Shell lint for this repository.
#
# Runs ShellCheck over every shell script of the repository: every '*.sh' file git does not
# ignore, and the git hooks under .githooks/, which git requires to be named without an extension.
# The files come from git, as in every other check, so a script under .scratch/ or under a build
# directory the project ignores is no part of the gate. The scripts under scripts/ and the hooks
# are the repository's entire enforcement layer, so they are held to a lint gate themselves.
#
# ShellCheck is the one check dependency beyond bash + coreutils/git, held to the one version
# scripts/install-shellcheck.sh pins (ADR-0044), which the Dev Container installs on creation
# through scripts/install-tools.sh and the shell job of CI before the lint, so that a script
# passes or fails alike in both. Elsewhere, when shellcheck is not on PATH, the check skips itself
# with a notice instead of failing (CI remains the enforcing gate); in a CI run (CI=true) a
# missing shellcheck is an error, never a silent skip. A shellcheck of another version names both
# versions; in a CI run it fails the lint, elsewhere it runs with a warning.
#
# Beyond ShellCheck it refuses GNU sed's in-place flag, `sed -i`, which BSD sed on macOS reads as
# taking a suffix: check-all.sh, and with it the pre-push hook, runs on any host. Comment lines are
# not read. The rule reads the options right after `sed`, so it misses a flag after a script
# (`sed -e S -i f`), and it reads text, so it flags the words inside a quoted string or after a
# trailing comment; spell those differently.
#
# Usage:
#     scripts/check-shell.sh [repository root]        (or: bash scripts/check-shell.sh)
# The root defaults to this repository; scripts/test-check-shell.sh passes fixtures.
# Exit code 0 when all scripts pass (ShellCheck skipped when unavailable outside CI), 1 otherwise,
# 2 on a root that is no directory.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Shell lint: '${1:-}' is not a directory." >&2; exit 2; }

have_shellcheck=1
if ! command -v shellcheck >/dev/null 2>&1; then
  if [[ "${CI:-}" == "true" ]]; then
    echo "ERROR: shellcheck not found in a CI run — scripts/install-shellcheck.sh installs it." >&2
    exit 1
  fi
  have_shellcheck=0
fi

# The version scripts/install-shellcheck.sh pins, beside this script, not under the root it lints.
pinned="$(sed -n 's/^SHELLCHECK_VERSION="\(.*\)"$/\1/p' \
  "$(dirname "${BASH_SOURCE[0]}")/install-shellcheck.sh" 2>/dev/null)"
if ((have_shellcheck)) && [[ -n "$pinned" ]]; then
  running="$(shellcheck --version 2>/dev/null | sed -n 's/^version: //p')"
  if [[ "$running" != "$pinned" ]]; then
    echo "Shell lint: shellcheck ${running:-of no version} runs, where $pinned is pinned" \
      "(scripts/install-shellcheck.sh installs it)." >&2
    if [[ "${CI:-}" == "true" ]]; then
      exit 1
    fi
  fi
fi

# A list git cannot give is an error, not an empty gate.
if ! listed="$(git -C "$ROOT" ls-files --cached --others --exclude-standard -- '*.sh' '.githooks/*')"; then
  echo "Shell lint: $ROOT is not a git repository — git lists the files the lint reads." >&2
  exit 1
fi
scripts=()
while IFS= read -r f; do
  [[ -n "$f" && -f "$ROOT/$f" ]] && scripts+=("$ROOT/$f")
done < <(sort -u <<< "$listed")

if ((${#scripts[@]} == 0)); then
  echo "No shell scripts found."
  exit 0
fi

# GNU-only: sed -i, with or without a suffix argument after it, and sed --in-place.
IN_PLACE='(^|[;&|(`[:space:]])sed([[:space:]]+-[a-zA-Z]+)*[[:space:]]+(-[a-zA-Z]*i([[:space:]]|$)|--in-place)'

failed=0
if ((have_shellcheck)); then
  shellcheck "${scripts[@]}" || failed=1
else
  echo "shellcheck is not installed — ShellCheck skipped (CI enforces it); the sed rule runs."
  echo "Install it to run locally: https://www.shellcheck.net"
fi

for f in "${scripts[@]}"; do
  while IFS=: read -r line text; do
    echo "${f#"$ROOT"/}:$line: GNU sed's in-place flag does not run on macOS;" \
      "write through a temporary file instead: sed 'SCRIPT' FILE > FILE.tmp && mv FILE.tmp FILE"
    echo "    $text"
    failed=1
  done < <(grep -nE "$IN_PLACE" "$f" | grep -vE '^[0-9]+:[[:space:]]*#')
done

if ((failed)); then
  echo
  echo "Shell lint FAILED (see findings above)."
  exit 1
fi

echo "Shell lint passed (${#scripts[@]} scripts)."
exit 0
