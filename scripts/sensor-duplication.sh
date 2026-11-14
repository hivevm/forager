#!/usr/bin/env bash
#
# Sensor: duplication (ADR-0038).
#
# Runs jscpd on the Rust and bash files git tracks, and reports each clone with its two places,
# then jscpd's table of the files, lines, and tokens read per format. The sensor names jscpd every
# tracked regular file, symbolic links and submodules left out, and jscpd keeps those of the
# formats .jscpd.json at the root names, a script without an extension, such as a git hook, by
# its shebang. An untracked file is no part of the report, so a local run reports what CI does on
# the same commit. The minimum clone size is that of .jscpd.json too: jscpd's defaults, 50 tokens
# and 5 lines. Its 'path' is the tree's root, which the named files narrow. The report lists every
# clone of the tree, those that predate the change among them.
#
# It reports and never fails on the count. It fails when it measures nothing: without
# .jscpd.json, outside a git repository, without the pin of scripts/install-jscpd.sh, without
# jscpd, with a jscpd of another version than that pin, or when jscpd fails or prints no count.
#
# Needs jscpd, which scripts/install-jscpd.sh installs; JSCPD names another binary, for the
# self-test. Usage:
#     scripts/sensor-duplication.sh [repository root]
# The root defaults to this repository; scripts/test-sensor-duplication.sh passes fixtures.
# Exit code 0 when jscpd ran and counted, 1 otherwise.

set -uo pipefail
# A caller's GIT_DIR, such as a hook's in a linked worktree, would turn this script's git on its
# repository instead of the one it names (docs/CONVENTIONS.md).
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "${1:-$(dirname "${BASH_SOURCE[0]}")/..}" && pwd)" || exit 1
JSCPD="${JSCPD:-jscpd}"

if [[ ! -f "$ROOT/.jscpd.json" ]]; then
  echo "Duplication sensor: $ROOT/.jscpd.json not found — it holds the formats, paths, and minimum clone size." >&2
  exit 1
fi

# The version scripts/install-jscpd.sh pins, beside this script, not under the root it reads.
installer="$(dirname "${BASH_SOURCE[0]}")/install-jscpd.sh"
pinned="$(sed -n 's/^JSCPD_VERSION="\(.*\)"$/\1/p' "$installer" 2>/dev/null)"
if [[ -z "$pinned" ]]; then
  echo "Duplication sensor: no pinned version found — $installer names none on a JSCPD_VERSION=\"…\" line." >&2
  exit 1
fi
if ! running="$("$JSCPD" --version 2>/dev/null)"; then
  echo "Duplication sensor: no jscpd found as '$JSCPD' — scripts/install-jscpd.sh installs it." >&2
  exit 1
fi
if [[ "$running" != "jscpd $pinned" ]]; then
  echo "Duplication sensor: ${running:-a jscpd of no version} runs, where jscpd $pinned is pinned" \
    "(scripts/install-jscpd.sh installs it)." >&2
  exit 1
fi

# The tracked regular files; a list git cannot give is an error, not an empty report.
if ! listed="$(git -C "$ROOT" ls-files --cached)"; then
  echo "Duplication sensor: $ROOT is not a git repository — git lists the files jscpd reads." >&2
  exit 1
fi
files=()
while IFS= read -r f; do
  [[ -n "$f" && -f "$ROOT/$f" && ! -L "$ROOT/$f" ]] && files+=("$f")
done <<< "$listed"
if ((${#files[@]} == 0)); then
  echo "Duplication sensor: 0 clones — git tracks no file to read."
  exit 0
fi

# jscpd names a file given by path by its base name alone, so it is given absolute paths, and the
# root is taken off them again.
report="$(cd "$ROOT" && "$JSCPD" --config .jscpd.json --no-tips --no-colors --absolute \
  "${files[@]/#/"$ROOT/"}" 2>&1)"
rc=$?
report="${report//"$ROOT/"/}"
count="$(sed -n 's/^Found \([0-9][0-9]*\) clones\{0,1\}\.$/\1/p' <<< "$report")"
if ((rc != 0)) || [[ -z "$count" ]]; then
  echo "Duplication sensor: jscpd exited $rc without a count of clones:" >&2
  printf '%s\n' "$report" >&2
  exit 1
fi

echo "Duplication sensor: $count clone$( ((count == 1)) || echo s) in the Rust and bash files, each with its two places ($running, .jscpd.json)."
# jscpd's own lines, without its count, its timing, and its note on the config it read.
grep -vE '^(Found [0-9]+ clones?\.|time: .*|Using config from .*|No duplicates found\.)$' <<< "$report"
exit 0
