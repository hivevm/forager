#!/usr/bin/env bash
#
# Self-test of the self-tests: every one that builds a fixture repository runs its git there,
# not in a repository a caller names through git's variables (docs/CONVENTIONS.md). A hook in a
# linked worktree sets GIT_DIR, and a fixture's 'git -C <dir>' does not override it.
#
# Each self-test that drives git runs with GIT_DIR naming a decoy repository; the decoy's refs
# and config must be unchanged afterwards, and the self-test must pass as it does without one.
#
# Usage:
#     scripts/test-fixture-git-env.sh
# Exit code 0 when every self-test leaves the decoy alone, 1 otherwise.

set -uo pipefail
# shellcheck disable=SC2046
unset $(git rev-parse --local-env-vars)

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SELF="$(basename "${BASH_SOURCE[0]}")"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

decoy="$work/decoy"
git init -q -b main "$decoy"
git -C "$decoy" -c user.name=t -c user.email=t@example.invalid commit -q --allow-empty -m decoy
state() { git -C "$decoy" for-each-ref; cat "$decoy/.git/config"; }
before="$(state)"

failed=0
passed=0
for test in "$ROOT"/scripts/test-*.sh; do
  name="$(basename "$test")"
  # Itself, and the normalize self-test, which runs every self-test in a copy of the tree.
  [[ "$name" == "$SELF" || "$name" == test-normalize-remap.sh ]] && continue
  grep -qE '\bgit ' "$test" || continue
  if ! GIT_DIR="$decoy/.git" bash "$test" > "$work/out" 2>&1; then
    failed=$((failed + 1))
    echo "FAIL: $name fails with GIT_DIR set" >&2
    sed 's/^/    /' "$work/out" >&2
  elif [[ "$(state)" != "$before" ]]; then
    failed=$((failed + 1))
    echo "FAIL: $name changed the repository GIT_DIR names" >&2
  else
    passed=$((passed + 1))
  fi
  # Each self-test is judged on what it changed itself.
  before="$(state)"
done

if ((failed)); then
  echo "Fixture git self-test FAILED ($failed of $((passed + failed)) self-tests)." >&2
  exit 1
fi
echo "Fixture git self-test passed ($passed self-tests)."
exit 0
