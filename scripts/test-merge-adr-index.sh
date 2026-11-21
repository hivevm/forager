#!/usr/bin/env bash
#
# Self-test of the ADR index's merge driver, .githooks/merge-adr-index, as .gitattributes names it
# for docs/adr/README.md (taken from NUC v0.7.0).
#
#   On a copy of the repository's files: two branches from the same main that each add an ADR and
#   its row merge without a conflict on the index, and scripts/check-docs.sh holds the result.
#
#   The cases on a small index, the rows both sides appended, a rule one or both sides changed, a
#   line changed right below a row, and the merge without the driver, are the template's, in
#   scripts/test-git-hooks.sh. That a row both sides changed comes out twice and fails
#   check-docs.sh is decided by scripts/test-check-docs.sh.
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

g() { local d="$1"; shift; git -C "$d" -c user.name=t -c user.email=t@example.invalid "$@"; }

repo="$work/tree"
mkdir -p "$repo"
(cd "$ROOT" && git ls-files --cached --others --exclude-standard | while IFS= read -r f; do
  [[ -e "$f" || -L "$f" ]] && printf '%s\0' "$f"
done | tar cf - --null -T -) | tar xf - -C "$repo"
git init -q -b main "$repo"
git -C "$repo" config merge.adr-index.driver "$DRIVER"
g "$repo" add -A && g "$repo" commit -q -m base

# add_adr <slug> — a proposed ADR, OPS-fixture-<slug>, and its row at the end of the Binding table.
add_adr() {
  local id="OPS-fixture-$1" slug="$1" applies="\`docs/$1.md\`" index="$repo/docs/adr/README.md" end
  printf '# %s: A decision of the fixture, %s\n\n- **Status:** 🟡 proposed\n- **Date:** 2026-11-07\n- **Deciders:** Fixture\n- **Applies to:** %s\n\n## Context\n\nA fixture.\n\n## Decision\n\nWe will test the merge driver.\n\n## Alternatives considered\n\n- None.\n\n## Sources / Prior art\n\n- None.\n\n## Consequences\n\n- None.\n\n## Enforcement\n\n**Not mechanically decidable:** a fixture.\n' \
    "$id" "$slug" "$applies" > "$repo/docs/adr/$id.md"
  # The last row above the heading of the dead ADRs ends the Binding table.
  end="$(awk '/^### Superseded and rejected/ { exit } /^\|/ { n = NR } END { print n }' "$index")"
  awk -v row="| [$id]($id.md) | A decision of the fixture, $slug | $applies | 🟡 proposed |" \
    -v end="$end" '{ print } NR == end { print row }' "$index" > "$index.tmp" && mv "$index.tmp" "$index"
}

g "$repo" switch -q -c one
add_adr one
g "$repo" add -A && g "$repo" commit -q -m one
g "$repo" switch -q -c two main
add_adr two
g "$repo" add -A && g "$repo" commit -q -m two
g "$repo" switch -q main
g "$repo" merge -q --no-ff -m "merge one" one
if g "$repo" merge -q --no-ff -m "merge two" two > /dev/null 2>&1; then
  passed=$((passed + 1))
  # Without the branch CI names: the fixture's merge is into its own main, which no check of the
  # branch a pull request names is to read.
  if GITHUB_BASE_REF="" GITHUB_REF_NAME="" GITHUB_ACTIONS="" \
    bash "$repo/scripts/check-docs.sh" --root "$repo" > "$work/out" 2>&1; then
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
