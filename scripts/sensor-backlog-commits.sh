#!/usr/bin/env bash
#
# Sensor: commits that add a backlog entry beside other changes (ADR-0005 — a session creates a
# backlog entry in a commit of its own, so a branch that is dropped can hand the entry on).
#
# Lists, between a base and a head, each commit that adds a file under 'docs/backlog/' and
# changes a path outside it, with the entries it adds and the paths beside them. Such a commit
# cannot be picked onto another branch without what lies beside the entry. It reports and never
# fails; the reviewer asks for the split. A commit that deletes an entry beside its ticket's work
# is not listed: the commit that takes up an entry deletes it. Nor is a commit whose only change
# outside the backlog deletes files under 'docs/plan/': a ticket that stopped, turned into its
# entry, is that form. Renames are not followed, so a file moved into the backlog reads as an
# entry added beside the path it left. Merge commits are not read; were one read, its diff to its
# first parent would be, so its self-test can tell.
#
# Pure bash + git. Usage:
#     scripts/sensor-backlog-commits.sh [--root <dir>] [base [head]]
# It reads the repository it lives in, as the checks do; --root names another, as its self-test
# does with each fixture.
# Without a base, the merge base of head with origin/main (else main) is used; CI passes the
# pull request's base. A base of zeros, which a push event reports for a new branch, counts as
# none. With nothing to compare — on main without a remote, or base and head the same — it says
# so. Exit code 0 always, except 2 on a base or head that does not resolve.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/git.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/git.sh"

enter_root "Backlog commits sensor" "${BASH_SOURCE[0]}" "$@"
shift "$ROOT_TAKEN"

resolve_change "Backlog commits sensor" "${1:-}" "${2:-HEAD}" || exit 0
base="$CHANGE_BASE" head="$CHANGE_HEAD"
if [[ "$(git rev-parse "$base^{commit}")" == "$(git rev-parse "$head^{commit}")" ]]; then
  echo "Backlog commits sensor: nothing to compare — base and head are the same commit."
  exit 0
fi

findings=()
while IFS=$'\t' read -r sha subject; do
  entries=() beside=()
  # Status and path, NUL-separated, as a path may hold any character.
  while IFS= read -r -d '' status && IFS= read -r -d '' path; do
    if [[ "$path" == docs/backlog/* ]]; then
      [[ "$status" == A ]] && entries+=("$path")
    elif [[ "$status" == D && "$path" == docs/plan/* ]]; then
      continue # a ticket's file, deleted as its entry takes its place
    else
      beside+=("$path")
    fi
  done < <(git diff-tree -r --root --no-commit-id --no-renames --name-status --diff-merges=first-parent -z "$sha")
  ((${#entries[@]} && ${#beside[@]})) || continue
  findings+=("$(git rev-parse --short "$sha") $subject — adds ${entries[*]} beside: ${beside[*]}")
done < <(git log --no-merges --reverse --format='%H%x09%s' "$base..$head")

short_base="$(git rev-parse --short "$base")"
short_head="$(git rev-parse --short "$head")"
if ((${#findings[@]} == 0)); then
  echo "Backlog commits sensor ($short_base..$short_head): nothing to report."
  exit 0
fi
echo "Backlog commits sensor ($short_base..$short_head): ${#findings[@]} commit(s) that add a backlog entry beside other changes than a ticket deleted — ADR-0005 has a session create each entry in a commit of its own, so a dropped branch can hand it on:"
printf '  - %s\n' "${findings[@]}"
exit 0
