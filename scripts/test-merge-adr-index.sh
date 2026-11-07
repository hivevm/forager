#!/usr/bin/env bash
#
# Self-test of the ADR index's merge driver, .githooks/merge-adr-index, as .gitattributes names it
# for docs/adr/README.md (taken from NUC v0.7.0).
#
#   - On a small index: the rows both sides appended stand after the merge; a rule one side changed
#     is taken; a rule both sides changed, or a line changed right below a row the other side
#     appended, stops the merge with conflict markers; without the driver, the rows conflict.
#   - On a copy of the repository's files: two branches from the same main that each add an ADR
#     and its row merge without a conflict on the index, and scripts/check-docs.sh holds the
#     result. That a row both sides changed comes out twice and fails check-docs.sh is decided by
#     scripts/test-check-docs.sh.
#
# Usage:
#     scripts/test-merge-adr-index.sh
# Exit code 0 when every case holds, 1 otherwise.

set -uo pipefail
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DRIVER="$ROOT/.githooks/merge-adr-index %O %A %B %S %X %Y"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0
check() {
  local name="$1"; shift
  if "$@"; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: $name" >&2
  fi
}

g() { local d="$1"; shift; git -C "$d" -c user.name=t -c user.email=t@example.invalid "$@"; }

# edit_index <file> <rule> <row> <after> — sets the rule, appends the row at the end of the table,
# and sets the line right below the table.
edit_index() {
  awk -v rule="$2" -v row="$3" -v after="$4" '
    /^Rule:/ { print "Rule: " rule "."; next }
    /^After:/ { print row; print "After: " after "."; next }
    { print }
  ' "$1" > "$1.tmp" && mv "$1.tmp" "$1"
}

# indexed <name> <rule on one> <rule on two> <line below the table on two> [none] — a repository
# whose index holds a rule, a row, and a line right below the table, with the repository's
# .gitattributes; branches 'one' and 'two' each append a row and set the rule as told, and 'two'
# the line below. The driver is configured unless the fifth argument is 'none'. Prints its path,
# checked out at 'two'.
indexed() {
  local d="$work/$1" index
  index="$d/docs/adr/README.md"
  mkdir -p "$d/docs/adr"
  git init -q -b main "$d"
  [[ "${5:-}" == none ]] || git -C "$d" config merge.adr-index.driver "$DRIVER"
  cp "$ROOT/.gitattributes" "$d/.gitattributes"
  printf 'Rule: one.\n\n| ADR |\n|-----|\n| [0001](0001-a.md) |\nAfter: one.\n' > "$index"
  g "$d" add -A && g "$d" commit -q -m index
  g "$d" switch -q -c one
  edit_index "$index" "$2" "| [0002](0002-b.md) |" one
  g "$d" commit -q -am one
  g "$d" switch -q -c two main
  edit_index "$index" "$3" "| [0003](0003-c.md) |" "$4"
  g "$d" commit -q -am two
  echo "$d"
}

# rows <dir> — the rule, the rows, and the line below the table, on one line.
rows() {
  grep -E '^(\| \[|Rule:|After:)' "$1/docs/adr/README.md" | tr '\n' ' '
}

# Both are called through check.
# shellcheck disable=SC2317
merges() { g "$1" merge -q -m m one > /dev/null 2>&1; }
# shellcheck disable=SC2317
conflicts() { ! merges "$1" && grep -q '^<<<<<<< ' "$1/docs/adr/README.md"; }

# --- The driver on a small index. ---
d="$(indexed rows one one one)"
check "the driver keeps the rows both sides appended" merges "$d"
check "both rows stand in the index after the merge" \
  test "$(rows "$d")" = "Rule: one. | [0001](0001-a.md) | | [0003](0003-c.md) | | [0002](0002-b.md) | After: one. "

d="$(indexed prose-one two one one)"
check "the driver takes a rule one side changed" merges "$d"
check "the rule one side changed stands once" \
  test "$(rows "$d")" = "Rule: two. | [0001](0001-a.md) | | [0003](0003-c.md) | | [0002](0002-b.md) | After: one. "

d="$(indexed prose-both two three one)"
check "the driver stops on a rule both sides changed, with markers" conflicts "$d"

d="$(indexed prose-adjacent one one two)"
check "the driver stops on a line changed right below a row the other side appended" conflicts "$d"

d="$(indexed no-driver one one one none)"
check "without the driver the rows conflict" conflicts "$d"

# --- The ticket's case on the repository's own files: two ADRs added in parallel. ---

repo="$work/tree"
mkdir -p "$repo"
(cd "$ROOT" && git ls-files --cached --others --exclude-standard | while IFS= read -r f; do
  [[ -e "$f" || -L "$f" ]] && printf '%s\0' "$f"
done | tar cf - --null -T -) | tar xf - -C "$repo"
git init -q -b main "$repo"
git -C "$repo" config merge.adr-index.driver "$DRIVER"
g "$repo" add -A && g "$repo" commit -q -m base

last="$(find "$repo/docs/adr" -name '[0-9][0-9][0-9][0-9]-*.md' | sed 's|.*/||' | sort | tail -1)"
last=$((10#${last%%-*}))

# add_adr <number> <slug> — a proposed ADR and its row at the end of the Binding table.
add_adr() {
  local number="$1" slug="$2" applies="\`docs/$2.md\`" index="$repo/docs/adr/README.md" end
  printf '# ADR-%s: A decision of the fixture, %s\n\n- **Status:** 🟡 proposed\n- **Date:** 2026-11-07\n- **Deciders:** Fixture\n- **Applies to:** %s\n\n## Context\n\nA fixture.\n\n## Decision\n\nWe will test the merge driver.\n\n## Alternatives considered\n\n- None.\n\n## Sources / Prior art\n\n- None.\n\n## Consequences\n\n- None.\n\n## Enforcement\n\n**Not mechanically decidable:** a fixture.\n' \
    "$number" "$slug" "$applies" > "$repo/docs/adr/$number-$slug.md"
  # The last row above the heading of the dead ADRs ends the Binding table.
  end="$(awk '/^### Superseded and rejected/ { exit } /^\|/ { n = NR } END { print n }' "$index")"
  awk -v row="| [$number]($number-$slug.md) | A decision of the fixture, $slug | $applies | 🟡 proposed |" \
    -v end="$end" '{ print } NR == end { print row }' "$index" > "$index.tmp" && mv "$index.tmp" "$index"
}

g "$repo" switch -q -c one
add_adr "$(printf '%04d' $((last + 1)))" one
g "$repo" add -A && g "$repo" commit -q -m one
g "$repo" switch -q -c two main
add_adr "$(printf '%04d' $((last + 2)))" two
g "$repo" add -A && g "$repo" commit -q -m two
g "$repo" switch -q main
g "$repo" merge -q --no-ff -m "merge one" one
if g "$repo" merge -q --no-ff -m "merge two" two > /dev/null 2>&1; then
  passed=$((passed + 1))
  if bash "$repo/scripts/check-docs.sh" "$repo" > "$work/out" 2>&1; then
    passed=$((passed + 1))
  else
    failed=$((failed + 1))
    echo "FAIL: the index two branches merged does not hold check-docs.sh:" >&2
    sed 's/^/    /' "$work/out" >&2
  fi
else
  failed=$((failed + 1))
  echo "FAIL: the second branch's ADR row conflicts on the index" >&2
fi

if ((failed)); then
  echo "ADR index merge self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "ADR index merge self-test passed ($passed cases)."
exit 0
