#!/usr/bin/env bash
#
# Self-test of scripts/check-inactive-adr-refs.sh, Forager's own check of the citations of
# superseded and rejected ADRs outside the record.
#
# Each case builds a directory of ADRs and text files, runs the check against it, and asserts the
# exit code and, for a failing case, the key and the data of the violation. The cases stand in
# scripts/fixtures/check-inactive-adr-refs.txt, which says its format. A line of the table that is
# neither blank, a comment, nor a row the reader reads fails, named (lib/rows.sh).
#
# Inputs: scripts/check-inactive-adr-refs.sh scripts/lib/ scripts/fixtures/
#
# Usage:
#     scripts/test-check-inactive-adr-refs.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn the fixtures' git on its
# repository; they run on their own (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/rows.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/rows.sh"
CHECK="$ROOT/scripts/check-inactive-adr-refs.sh"
TABLE="$ROOT/scripts/fixtures/check-inactive-adr-refs.txt"
ROW_RE='^(pass|fail) \| '

# status_line <status> — the Status line of a step 'adr'.
status_line() {
  case "$1" in
    accepted) echo "🟢 accepted" ;;
    proposed) echo "🟡 proposed" ;;
    rejected) echo "🔴 rejected" ;;
    superseded) echo "⚪ superseded" ;;
    superseded:*) echo "⚪ superseded by [${1#superseded:}](${1#superseded:}.md)" ;;
    *) echo "fixture table: unknown status '$1'" >&2; exit 1 ;;
  esac
}

table_rows "$ROW_RE" < "$TABLE" > "$work/rows"
n=0
while IFS= read -r line; do
  n=$((n + 1))
  IFS=$'\x1f' read -r want needle name tree <<< "$(table_hyphens "${line// | /$'\x1f'}")"
  d="$work/case-$n"
  mkdir -p "$d"
  [[ "$tree" != - ]] || tree=""
  while IFS= read -r s; do
    [[ -n "$s" ]] || continue
    read -r op arg <<< "$s"
    case "$op" in
      adr)
        mkdir -p "$d/docs/adr"
        printf '# %s: decision\n\n- **Status:** %s\n' "${arg%% *}" "$(status_line "${arg#* }")" \
          > "$d/docs/adr/${arg%% *}.md" ;;
      category)
        mkdir -p "$d/docs/adr"
        [[ -f "$d/docs/adr/README.md" ]] || echo '### Categories' > "$d/docs/adr/README.md"
        echo "- **$arg** — a category." >> "$d/docs/adr/README.md" ;;
      text)
        mkdir -p "$(dirname "$d/${arg%% *}")"
        printf '%s\n' "${arg#* }" >> "$d/${arg%% *}" ;;
      git) git -C "$d" init -q && git -C "$d" add -A ;;
      *) echo "fixture table: unknown step '$op'" >&2; exit 1 ;;
    esac
  done <<< "${tree//; /$'\n'}"
  out="$(bash "$CHECK" --root "$d" 2>&1)"
  table_case "$name" "$want" "$needle" "$?" "$out"
done < "$work/rows"
((n > 0)) || { echo "FAIL: no case read from $TABLE" >&2; failed=$((failed + 1)); }

out="$(bash "$CHECK" --root "$work/absent" 2>&1)"
check "a root that is no directory is refused" test "$?" -eq 2

summary "Inactive ADR references self-test"
