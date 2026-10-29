#!/usr/bin/env bash
#
# Self-test of how MapServer runs on CI. Verifies: ADR-0027
#
# The rules hold scripts/build-mapserver.sh and .github/workflows/noise.yml: the build script
# reads the pin from scripts/fetch-msautotest.sh and names no tag or commit of its own, refuses to
# run outside GitHub Actions, and neither file adds an apt source; the workflow runs on
# ubuntu-24.04 without a container, triggers on demand and on a push to the paths of the ADR only,
# and keys the cache of the build on the two scripts alone. The real files must pass; each case
# then breaks one rule in a copy and must fail with the line that names it.
#
# Usage:
#     scripts/test-build-mapserver.sh
# Exit code 0 when every case passes, 1 otherwise.

# The patterns and sed expressions of this file name the shell's $ROOT, $TAG, and $PIN literally.
# shellcheck disable=SC2016

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD="$ROOT/scripts/build-mapserver.sh"
WORKFLOW="$ROOT/.github/workflows/noise.yml"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# The trigger block of the workflow as the ADR decides it, compared line by line.
TRIGGER='on:
  workflow_dispatch:
  push:
    paths:
      - "crates/cli/tests/conformance/**"
      - "crates/cli/tests/metric/**"
      - "crates/cli/tests/msautotest.rs"
      - "scripts/fetch-msautotest.sh"
      - "scripts/build-mapserver.sh"
      - ".github/workflows/noise.yml"'
CACHE_KEY="key: mapserver-\${{ hashFiles('scripts/build-mapserver.sh', 'scripts/fetch-msautotest.sh') }}"

# Stand-ins for every command a build runs, each failing loudly: the refusal probe runs the build
# script for real, and a copy whose refusal is gone must not install, clone, or build anything.
stubs="$work/stubs"
mkdir -p "$stubs"
for tool in sudo apt-get git cmake dpkg-query apt-cache ldd; do
  printf '#!/bin/sh\necho "stub %s reached" >&2\nexit 97\n' "$tool" > "$stubs/$tool"
  chmod +x "$stubs/$tool"
done

# check <build script> <workflow> — prints one line per rule broken; exit 1 when one is.
check() {
  local build="$1" workflow="$2" problems=()
  if grep -Eq '^[[:space:]]*(TAG|COMMIT)=["'"'"']?[^"$]' "$build" \
    || grep -Eq '\b[0-9a-f]{7,40}\b|rel-[0-9]+-[0-9]+-[0-9]+' "$build" \
    || grep -E -- 'git[^|]* (clone|checkout|fetch)|--branch' "$build" \
      | grep -vq -- '--branch "\$TAG"'; then
    problems+=("the build script names a tag or a commit of its own")
  fi
  if ! grep -qx 'PIN="\$ROOT/scripts/fetch-msautotest.sh"' "$build" \
    || [[ "$(grep -cE "^(TAG|COMMIT|REMOTE)=\"\\\$\(sed -n 's/\^(TAG|COMMIT|REMOTE)=.*\"\\\$PIN\"\)\"$" "$build")" != 3 ]]; then
    problems+=("the build script does not read the pin of scripts/fetch-msautotest.sh")
  fi
  local out rc
  out="$(env -u GITHUB_ACTIONS PATH="$stubs:$PATH" bash "$build" 2>&1 </dev/null)"; rc=$?
  if ((rc != 2)) || [[ "$out" != *"runs on a GitHub-hosted runner only"* ]]; then
    problems+=("the build script does not refuse to run outside GitHub Actions")
  fi
  local file
  for file in "$build" "$workflow"; do
    if grep -Eq 'add-apt-repository|apt-add-repository|sources\.list' "$file"; then
      problems+=("$(basename "$file") adds an apt source")
    fi
  done
  if [[ "$(grep -E '^[[:space:]]*runs-on:' "$workflow" | sed 's/^[[:space:]]*//' | sort -u)" \
    != "runs-on: ubuntu-24.04" ]]; then
    problems+=("the workflow runs on another runner than ubuntu-24.04")
  fi
  if grep -Eq '^[[:space:]]*(container|services):|docker|podman' "$workflow"; then
    problems+=("the workflow uses a container")
  fi
  if [[ "$(sed -n '/^on:/,/^[^[:space:]o]/p' "$workflow" | sed '/^[^[:space:]o]/d;/^$/d')" \
    != "$TRIGGER" ]]; then
    problems+=("the workflow triggers on more than the ADR's push paths and workflow_dispatch")
  fi
  if [[ "$(grep -A1 -E '^[[:space:]]*path: target/mapserver$' "$workflow" | sed -n '2s/^[[:space:]]*//p')" \
    != "$CACHE_KEY" ]] || (($(grep -c 'target/mapserver' "$workflow") != 1)); then
    problems+=("the workflow keys the build's cache on more or less than the two scripts")
  fi
  ((${#problems[@]} == 0)) && return 0
  printf '%s\n' "${problems[@]}"
  return 1
}

# expect <name> <build> <workflow> <code> <needle>
expect() {
  local name="$1" build="$2" workflow="$3" code="$4" needle="$5"
  local out rc
  out="$(check "$build" "$workflow" 2>&1)"; rc=$?
  if ((rc == code)) && [[ "$out" == *"$needle"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $code with \"$needle\", exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

# copy <name> <sed expression for the build script> <sed expression for the workflow> — a pair of
# changed copies, printed as two paths on one line.
copy() {
  mkdir -p "$work/$1"
  sed "$2" "$BUILD" > "$work/$1/build-mapserver.sh"
  sed "$3" "$WORKFLOW" > "$work/$1/noise.yml"
  echo "$work/$1/build-mapserver.sh $work/$1/noise.yml"
}

expect "the build script and the workflow pass" "$BUILD" "$WORKFLOW" 0 ""

read -r b w <<< "$(copy tag 's/^PIN=.*/&\nTAG="rel-8-6-6"/' '')"
expect "a tag of the build script's own fails" "$b" "$w" 1 "names a tag or a commit"

read -r b w <<< "$(copy commit 's/^PIN=.*/&\nCOMMIT="7515cccfe75fb7554ea45bc87ba5eb4b6126db5d"/' '')"
expect "a commit of the build script's own fails" "$b" "$w" 1 "names a tag or a commit"

read -r b w <<< "$(copy nopin 's/fetch-msautotest\.sh/pin.sh/g' '')"
expect "a build script that does not read the pin fails" "$b" "$w" 1 "does not read the pin"

read -r b w <<< "$(copy pincomment 's/^PIN=.*/PIN="$ROOT\/scripts\/pin.sh"/' '')"
expect "a pin read from elsewhere, the name left in a comment, fails" "$b" "$w" 1 "does not read the pin"

read -r b w <<< "$(copy branch 's/--branch "\$TAG"/--branch 8.6.6/' '')"
expect "a branch of the build script's own fails" "$b" "$w" 1 "names a tag or a commit"

read -r b w <<< "$(copy short 's/^PIN=.*/&\nX=7515ccc/' '')"
expect "an abbreviated commit fails" "$b" "$w" 1 "names a tag or a commit"

read -r b w <<< "$(copy runs '/GITHUB_ACTIONS:-/,/^fi$/d' '')"
expect "a build script that runs anywhere fails" "$b" "$w" 1 "does not refuse"

read -r b w <<< "$(copy ppa 's/^sudo apt-get update.*/sudo add-apt-repository -y ppa:ubuntugis\/ubuntugis-unstable\n&/' '')"
expect "a PPA in the build script fails" "$b" "$w" 1 "build-mapserver.sh adds an apt source"

read -r b w <<< "$(copy alias 's/^sudo apt-get update.*/sudo apt-add-repository -y ppa:x\/y\n&/' '')"
expect "the apt-add-repository alias fails" "$b" "$w" 1 "build-mapserver.sh adds an apt source"

read -r b w <<< "$(copy list '' 's/^      - name: Build MapServer.*/      - run: echo deb x | sudo tee \/etc\/apt\/sources.list.d\/x.list\n&/')"
expect "a source written by the workflow fails" "$b" "$w" 1 "noise.yml adds an apt source"

read -r b w <<< "$(copy runner '' 's/runs-on: ubuntu-24.04/runs-on: ubuntu-latest/')"
expect "another runner fails" "$b" "$w" 1 "another runner"

read -r b w <<< "$(copy container '' 's/^    runs-on: ubuntu-24.04/&\n    container: ubuntu:24.04/')"
expect "a container job fails" "$b" "$w" 1 "uses a container"

read -r b w <<< "$(copy docker '' 's/^      - name: Build MapServer.*/      - run: docker run ubuntu:24.04 true\n&/')"
expect "a docker step fails" "$b" "$w" 1 "uses a container"

read -r b w <<< "$(copy services '' 's/^    runs-on: ubuntu-24.04/&\n    services:\n      db:\n        image: x/')"
expect "a service container fails" "$b" "$w" 1 "uses a container"

read -r b w <<< "$(copy podman '' 's/^      - name: Build MapServer.*/      - run: podman run x true\n&/')"
expect "a podman step fails" "$b" "$w" 1 "uses a container"

read -r b w <<< "$(copy schedule '' 's/^  workflow_dispatch:/&\n  schedule:\n    - cron: "0 0 * * 0"/')"
expect "a schedule fails" "$b" "$w" 1 "triggers on more"

read -r b w <<< "$(copy everypush '' '/^    paths:/,/noise\.yml"$/d')"
expect "a push without paths fails" "$b" "$w" 1 "triggers on more"

read -r b w <<< "$(copy pr '' 's/^  push:/  pull_request:\n&/')"
expect "a pull_request trigger fails" "$b" "$w" 1 "triggers on more"

read -r b w <<< "$(copy key '' "s/'scripts\/build-mapserver.sh', //")"
expect "a cache key without the build script fails" "$b" "$w" 1 "keys the build's cache"

read -r b w <<< "$(copy second '' 's/^      - name: Build MapServer.*/      - uses: actions\/cache@v6\n        with:\n          path: target\/mapserver\n          key: other\n&/')"
expect "a second cache of the build fails" "$b" "$w" 1 "keys the build's cache"

echo "build-mapserver self-test: $passed passed, $failed failed."
((failed == 0))
