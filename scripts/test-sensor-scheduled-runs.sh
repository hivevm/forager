#!/usr/bin/env bash
#
# Self-test of scripts/sensor-scheduled-runs.sh.
#
# The sensor asks gh for the latest scheduled run on main of each workflow that runs on a schedule;
# here a stand-in named by GH answers instead. It prints, for 'run list', the lines gh's --jq would
# print for the runs of one workflow, newest first, kept in a fixture directory, as many as --limit
# asks for, and fails unless the query names the branch main and the event schedule. Each case
# writes the workflows and those answers, runs the sensor, and asserts the exit code and a line the
# reviewer would read. What the --jq expression makes of GitHub's JSON is not run here, since it
# needs gh; that is review.
#
# Verifies: ADR-0002
# Inputs: scripts/sensor-scheduled-runs.sh scripts/lib/
#
# Usage:
#     scripts/test-sensor-scheduled-runs.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
SENSOR="$ROOT/scripts/sensor-scheduled-runs.sh"

# The stand-in for gh: 'repo view' prints the repository, 'run list' the first --limit lines of
# the fixture file of its --workflow, 20 without one as gh's default, and fails where there is no
# such file or the query is not for main's scheduled runs. Every call is logged to $FIX/calls.
fake="$work/gh"
cat > "$fake" <<'EOF'
#!/usr/bin/env bash
echo "$*" >> "$FIX/calls"
case "$1 $2" in
  "repo view") [[ -f "$FIX/repo" ]] && cat "$FIX/repo" ;;
  "run list")
    shift 2
    workflow="" branch="" event="" limit=20
    while (($#)); do
      case "$1" in
        --workflow | -w) workflow="$2"; shift ;;
        --branch | -b) branch="$2"; shift ;;
        --event | -e) event="$2"; shift ;;
        --limit | -L) limit="$2"; shift ;;
        --json | --jq | -q | --repo | -R) shift ;;
      esac
      shift
    done
    [[ "$branch" == main && "$event" == schedule && -f "$FIX/runs_$workflow" ]] || exit 1
    head -n "$limit" "$FIX/runs_$workflow"
    ;;
  *) exit 1 ;;
esac
EOF
chmod +x "$fake"

URL="https://github.com/owner/name/actions/runs"
# One run as the sensor's --jq prints it: conclusion, status, creation time, and URL.
run() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4"; }

# repo <name> — a fixture with three workflows on a schedule, Checks, Fuzz, and Mutants, and one
# without, Release, whose latest scheduled runs on main all succeeded. Printed as its path; a case
# edits the answers under <path>/fix.
repo() {
  local dir="$work/$1" w
  mkdir -p "$dir/fix" "$dir/.github/workflows"
  printf 'name: Checks\non:\n  push:\n  schedule:\n    - cron: "17 5 * * 1"\n' \
    > "$dir/.github/workflows/checks.yml"
  printf 'name: Fuzz\n\non:\n  schedule:\n    - cron: "41 3 * * 0"\n  workflow_dispatch:\n' \
    > "$dir/.github/workflows/fuzz.yml"
  printf "name: 'Mutants'\non:\n  schedule:\n    - cron: \"17 4 * * 0\"\n" \
    > "$dir/.github/workflows/mutants.yml"
  printf '# Runs on a tag; no schedule.\nname: Release\non:\n  push:\n    tags: ["v*"]\n' \
    > "$dir/.github/workflows/release.yml"
  echo "owner/name" > "$dir/fix/repo"
  for w in checks fuzz mutants; do
    run success completed 2026-11-16T05:17:02Z "$URL/1$w" > "$dir/fix/runs_$w.yml"
  done
  echo "$dir"
}

# expect <name> <dir> <rc> <needle> [absent] — run the sensor; it must exit <rc> and its output
# contain the needle, and not the second needle when one is given.
expect() {
  local name="$1" dir="$2" want="$3" needle="$4" absent="${5:-}" out rc
  out="$(GH="${GH_CMD:-$fake}" FIX="$dir/fix" bash "$SENSOR" "$dir" 2>&1)"; rc=$?
  if ((rc == want)) && [[ "$out" == *"$needle"* ]] && [[ -z "$absent" || "$out" != *"$absent"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $want with \"$needle\", got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

d="$(repo green)"
expect "latest scheduled runs that all succeeded report nothing" "$d" 0 "nothing to report"
check "each workflow on a schedule is asked for its latest scheduled run on main alone" \
  test "$(grep -c -- '--branch main --event schedule --limit 1' "$d/fix/calls")" -eq 3
check "a workflow without a schedule is not asked" \
  bash -c "! grep -q -- 'release.yml' '$d/fix/calls'"

d="$(repo fuzz-failed)"
run failure completed 2026-11-15T03:41:22Z "$URL/2" > "$d/fix/runs_fuzz.yml"
expect "a failed latest scheduled run is listed with its workflow, date, and URL" "$d" 0 \
  "Fuzz (fuzz.yml): failure on 2026-11-15 — $URL/2" "Checks"
expect "a sensor that lists a failed run still exits 0" "$d" 0 "1 of 3 workflow(s)"

d="$(repo older-failure)"
{
  run success completed 2026-11-16T05:17:02Z "$URL/4"
  run failure completed 2026-11-09T05:17:02Z "$URL/3"
} > "$d/fix/runs_checks.yml"
expect "an older failed run before a later successful one lists nothing" "$d" 0 \
  "nothing to report" "$URL/3"

d="$(repo all-failed)"
run failure completed 2026-11-15T03:41:22Z "$URL/2" > "$d/fix/runs_fuzz.yml"
run startup_failure completed 2026-11-15T04:17:09Z "$URL/5" > "$d/fix/runs_mutants.yml"
run timed_out completed 2026-11-16T05:17:02Z "$URL/6" > "$d/fix/runs_checks.yml"
expect "a workflow name in quotes is read without them" "$d" 0 \
  "Mutants (mutants.yml): startup_failure on 2026-11-15 — $URL/5"
expect "every workflow whose latest scheduled run did not succeed is listed" "$d" 0 \
  "3 of 3 workflow(s)"

d="$(repo running)"
run "" in_progress 2026-11-16T05:17:02Z "$URL/7" > "$d/fix/runs_checks.yml"
expect "a latest scheduled run without a conclusion is listed by its status" "$d" 0 \
  "Checks (checks.yml): in_progress since 2026-11-16 — $URL/7"

d="$(repo never-ran)"
: > "$d/fix/runs_mutants.yml"
expect "a workflow without a scheduled run on main says so" "$d" 0 \
  "Mutants (mutants.yml): no scheduled run on main"

d="$(repo unreadable)"
rm "$d/fix/runs_fuzz.yml"
expect "a workflow whose runs gh cannot list says so" "$d" 0 \
  "Fuzz (fuzz.yml): its scheduled runs could not be read"

d="$(repo no-schedule)"
rm "$d/.github/workflows/"{checks,fuzz,mutants}.yml
expect "a repository without a workflow on a schedule has nothing to ask" "$d" 0 \
  "no workflow under .github/workflows runs on a schedule"

d="$(repo unauthenticated)"
rm "$d/fix/repo"
expect "a gh that cannot resolve the repository could not ask and exits 2" "$d" 2 "could not ask"

d="$(repo no-gh)"
GH_CMD="$work/no-such-gh" expect "no gh could not ask and exits 2" "$d" 2 "could not ask"

expect "a root that does not exist exits 2" "$work/missing" 2 "is not a directory"

summary "Scheduled runs sensor self-test"
