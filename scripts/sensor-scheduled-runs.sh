#!/usr/bin/env bash
#
# Sensor: scheduled runs on main.
#
# The workflows that run on a schedule, such as the weekly checks, fuzz run, and mutation testing,
# run on main without a pull request, so a red run reaches no one: a pull request shows only the
# checks of its own head. This sensor asks the GitHub CLI for the latest scheduled run on main of
# each workflow under .github/workflows/ whose triggers hold a 'schedule:', and lists each one that
# did not succeed, with its workflow, its date, and its URL (ADR-0002, sensors of the GitHub
# repository). An older failure before a later success is not listed: the latest run is the state.
# A workflow without a scheduled run on main is listed too, since GitHub disables a schedule after
# 60 days without activity in a public repository.
#
# It runs on a human's instruction, not in scripts/check-all.sh or checks.yml: a red scheduled run
# is no fault of the branch being checked, and a gate on the network would refuse work for a
# reason the work does not have. It reports and never fails. scripts/test-sensor-scheduled-runs.sh
# runs it on fixture responses: GH names the command that stands in for gh.
#
# Bash + coreutils + grep + sed, plus the GitHub CLI, authenticated through its web flow (AGENTS.md,
# section 6); the JSON is read by gh's own --jq. Usage:
#     scripts/sensor-scheduled-runs.sh [repository root]
# The repository is the one gh resolves from the root's git remote. Exit code 0 always, except
# 2 when the root does not exist, gh is missing, or gh cannot resolve the repository.

set -uo pipefail

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" 2>/dev/null && pwd)" \
  || { echo "Scheduled runs sensor: '${1:-}' is not a directory." >&2; exit 2; }
GH="${GH:-gh}"
NAME="Scheduled runs sensor"

command -v "$GH" >/dev/null 2>&1 \
  || { echo "$NAME: the GitHub CLI ($GH) is not installed, so it could not ask." >&2; exit 2; }
repo="$(cd "$ROOT" && "$GH" repo view --json nameWithOwner --jq .nameWithOwner 2>/dev/null)"
[[ -n "$repo" ]] \
  || { echo "$NAME: gh cannot resolve the repository of $ROOT, so it could not ask — authenticate with 'gh auth login', or add the remote." >&2; exit 2; }

# The workflows on a schedule, by file name, the name gh's --workflow takes.
mapfile -t workflows < <(grep -lE '^[[:space:]]+schedule:' \
  "$ROOT"/.github/workflows/*.yml "$ROOT"/.github/workflows/*.yaml 2>/dev/null | sort)
if ((${#workflows[@]} == 0)); then
  echo "$NAME: no workflow under .github/workflows runs on a schedule — nothing to ask."
  exit 0
fi

findings=()
for path in "${workflows[@]}"; do
  file="${path##*/}"
  # The workflow's name, its first top-level 'name:', without quotes; else its file name.
  label="$(sed -nE 's/^name:[[:space:]]*["'\'']?([^"'\'']*)["'\'']?[[:space:]]*$/\1/p' "$path" | head -n 1)"
  label="${label:-$file} ($file)"
  # The newest run first; --all keeps a workflow GitHub disabled. One line: conclusion, status,
  # creation time, and URL, split by a tab.
  line="$(cd "$ROOT" && "$GH" run list --repo "$repo" --all --workflow "$file" \
    --branch main --event schedule --limit 1 --json conclusion,status,createdAt,url \
    --jq '.[] | "\(.conclusion)\t\(.status)\t\(.createdAt)\t\(.url)"' 2>/dev/null)"
  rc=$?
  if ((rc)); then
    findings+=("$label: its scheduled runs could not be read")
    continue
  fi
  if [[ -z "$line" ]]; then
    findings+=("$label: no scheduled run on main")
    continue
  fi
  # Split by hand: read would merge the tabs around a run's empty conclusion.
  conclusion="${line%%$'\t'*}" line="${line#*$'\t'}"
  status="${line%%$'\t'*}" line="${line#*$'\t'}"
  date="${line%%T*}" url="${line#*$'\t'}"
  case "$conclusion" in
    success) ;;
    "") findings+=("$label: $status since $date — $url") ;;
    *) findings+=("$label: $conclusion on $date — $url") ;;
  esac
done

if ((${#findings[@]} == 0)); then
  echo "$NAME: the latest scheduled run on main of each of ${#workflows[@]} workflow(s) of $repo succeeded — nothing to report."
  exit 0
fi
echo "$NAME: ${#findings[@]} of ${#workflows[@]} workflow(s) of $repo whose latest scheduled run on main did not succeed:"
printf '  - %s\n' "${findings[@]}"
exit 0
