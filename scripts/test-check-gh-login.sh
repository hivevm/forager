#!/usr/bin/env bash
#
# Self-test of scripts/check-gh-login.sh, Forager's own check of the GitHub CLI login's mount.
#
# Each case writes a fixture with one .devcontainer/devcontainer.json, runs the check against it,
# and asserts the exit code and, for a failing case, the key and the data of the violation. The
# cases stand in scripts/fixtures/check-gh-login.txt, which says its format. A line of the table
# that is neither blank, a comment, nor a row the reader reads fails, named (lib/rows.sh).
#
# Inputs: scripts/check-gh-login.sh scripts/lib/ scripts/fixtures/check-gh-login.txt
#
# Usage:
#     scripts/test-check-gh-login.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/rows.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/rows.sh"
CHECK="$ROOT/scripts/check-gh-login.sh"
TABLE="$ROOT/scripts/fixtures/check-gh-login.txt"
ROW_RE='^(pass|fail) \| '

# shellcheck disable=SC2016 # the variable is devcontainer.json's, not the shell's
MOUNTS='[ "source=${localEnv:HOME}/.config/gh-devcontainer,target=/home/vscode/.config/gh,type=bind" ]'

# config <dir> <mounts, or -> [line...] — write devcontainer.json with the mounts and the lines.
config() {
  local dir="$1" mounts="$2"; shift 2
  mkdir -p "$dir/.devcontainer"
  {
    echo '{'
    echo '  "name": "fixture",'
    echo '  // The login of the GitHub CLI.'
    [[ "$mounts" == - ]] || echo "  \"mounts\": $mounts,"
    printf '  %s\n' "$@"
    echo '  "image": "mcr.microsoft.com/devcontainers/base:debian"'
    echo '}'
  } > "$dir/.devcontainer/devcontainer.json"
}

# Verifies: SEC-gh-login-outlives-rebuilds
table_rows "$ROW_RE" < "$TABLE" > "$work/rows"
n=0
while IFS= read -r line; do
  n=$((n + 1))
  IFS=$'\x1f' read -r want needle name tree <<< "${line// | /$'\x1f'}"
  d="$work/case-$n" mounts="$MOUNTS" lines=()
  config "$d" "$mounts"
  [[ "$tree" != - ]] || tree=""
  while IFS= read -r s; do
    [[ -n "$s" ]] || continue
    read -r op arg <<< "$s"
    case "$op" in
      mounts) mounts="$arg"; config "$d" "$mounts" "${lines[@]}" ;;
      line) lines+=("$arg"); config "$d" "$mounts" "${lines[@]}" ;;
      rm) rm -r "${d:?}/$arg" ;;
      *) echo "fixture table: unknown step '$op'" >&2; exit 1 ;;
    esac
  done <<< "${tree//; /$'\n'}"
  out="$(bash "$CHECK" --root "$d" 2>&1)"
  table_case "$name" "$want" "$needle" "$?" "$out"
done < "$work/rows"
((n > 0)) || { echo "FAIL: no case read from $TABLE" >&2; failed=$((failed + 1)); }

out="$(bash "$CHECK" --root "$work/absent" 2>&1)"
check "a root that is no directory is refused" test "$?" -eq 2

summary "GitHub CLI login self-test"
