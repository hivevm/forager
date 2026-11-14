# shellcheck shell=bash
# The project's self-tests' one counted assertion on a command, its exit code and a needle of its
# output, and the commit of their fixture repositories. Sourced by the project's own
# scripts/test-*.sh; the self-tests the template brought keep their copies, which a take-up
# replaces (ADR-0004).
#
# The sourcing self-test holds the counters, 'passed' and 'failed', and the temporary directory
# of its fixtures, 'work', as scripts/lib/selftest.sh defines them.
#
# Usage, from scripts/test-<name>.sh:
#     # shellcheck source-path=SCRIPTDIR source=lib/expect.sh
#     source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

# verdict <name> <code> <needle> <exit code> <output> [<also>] — count one check: it passes when
# the exit code is <code>, or any but 0 when <code> is 'nonzero', the output contains the needle,
# and not $REJECT when that is set, and <also>, the caller's own comparison as 1 or 0, is 1, which
# it is when not given. A failure is reported with the output, and returns 1, so that the caller
# can add what its own comparison saw.
verdict() {
  local name="$1" code="$2" needle="$3" rc="$4" out="$5" also="${6-1}"
  local rc_ok=0
  if [[ "$code" == nonzero ]]; then ((rc != 0)) && rc_ok=1; else ((rc == code)) && rc_ok=1; fi
  if ((rc_ok && also)) && [[ "$out" == *"$needle"* ]] \
    && [[ -z "${REJECT:-}" || "$out" != *"$REJECT"* ]]; then
    passed=$((passed + 1))
    return 0
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $code with \"$needle\"${REJECT:+ and without \"$REJECT\"}, got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
  return 1
}

# expect <name> <code> <needle> <command...> — run the command, its errors with its output, and
# count its verdict.
expect() {
  local name="$1" code="$2" needle="$3"
  shift 3
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  verdict "$name" "$code" "$needle" "$rc" "$out"
}

# commit <dir> <message> — stage everything in the fixture repository and commit with a fixed
# identity. It refuses a path that is no git repository under $work, as 'git -C ""' acts on this
# repository, and a path outside $work may be this repository or another one.
commit() {
  [[ -n "${1:-}" && "$1" == "${work:?}"/* && -d "$1/.git" ]] \
    || { echo "commit: '${1:-}' is no fixture repository under $work" >&2; exit 1; }
  git -C "${1:?}" add -A
  git -C "${1:?}" -c user.name=fixture -c user.email=fixture@example.invalid commit -q -m "$2"
}
