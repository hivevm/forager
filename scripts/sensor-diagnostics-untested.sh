#!/usr/bin/env bash
#
# Sensor: the diagnostic codes no test names (docs/CONVENTIONS.md — diagnostic codes are
# kebab-case and stable, and tests match on them).
#
# Lists each code written as a string literal into Diagnostic::warning or Diagnostic::error in the
# Rust sources under crates/*/src, on the line of the call or on the line after it, as rustfmt
# wraps a long call, and names each that no test names. A test is a file under a crate's tests/,
# the file of a module a '#[cfg(test)]' declaration names, such as src/tests.rs, or an inline
# '#[cfg(test)]' module, from its 'mod <name> {' line to the '}' that closes it at the start of a
# line, as rustfmt writes it. A test of any crate counts, and it names a code when it holds it
# between two characters that are no part of a kebab-case code. A code passed as a variable or
# through a helper of its own, such as the mapfile parser's 'error', is not read, nor is a call a
# test makes. It reports and never fails (NUC-checks-are-bash §4): a code no test names is one whose
# test the reviewer asks for, through the load or render that pushes it.
#
# Pure bash + grep + coreutils. Usage:
#     scripts/sensor-diagnostics-untested.sh [--root <dir>]
# It reads the repository it lives in, as the checks do; --root names another tree, as its
# self-test does with each fixture.
# Exit code 0, except 2 on an argument it does not read or a root that is not a directory.

set -uo pipefail
shopt -s nullglob globstar

# shellcheck source-path=SCRIPTDIR source=lib/bash.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/bash.sh"

# shellcheck source-path=SCRIPTDIR source=lib/root.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/root.sh"
usage="scripts/sensor-diagnostics-untested.sh [--root <dir>]"
enter_root "${BASH_SOURCE[0]}" "$usage" "$@"
rest_args "${BASH_SOURCE[0]}" "$usage" 0 "${ARGS[@]}"

NAME="Diagnostics untested sensor"

CALL='Diagnostic::(warning|error)\('
CODE='^[[:space:]]*"([a-z0-9][a-z0-9-]*)"'
CFG_TEST='^#\[cfg\(test\)\]$'
INLINE_MODULE='^(pub(\([a-z]+\))? )?mod [a-z_][a-z0-9_]* \{$'
MODULE_FILE='^(pub(\([a-z]+\))? )?mod ([a-z_][a-z0-9_]*);$'

inline="$(mktemp)"
trap 'rm -f "$inline"' EXIT

# module_files <file> <name> — the files that may hold the module <name> the file declares: beside
# it for a lib.rs, main.rs, or mod.rs, else in the directory of the file's own name.
module_files() {
  local dir="${1%/*}"
  case "${1##*/}" in
    lib.rs | main.rs | mod.rs) ;;
    *) dir="${1%.rs}" ;;
  esac
  printf '%s\n' "$dir/$2.rs" "$dir/$2/mod.rs"
}

# The module files the sources' '#[cfg(test)]' declarations name, each a test whole.
declare -A test_file=()
sources=(crates/*/src/**/*.rs)
for f in "${sources[@]}"; do
  mapfile -t text < "$f"
  for ((i = 0; i + 1 < ${#text[@]}; i++)); do
    [[ "${text[i]}" =~ $CFG_TEST && "${text[i + 1]}" =~ $MODULE_FILE ]] || continue
    while IFS= read -r m; do
      [[ -f "$m" ]] && test_file["$m"]=1
    done < <(module_files "$f" "${BASH_REMATCH[3]}")
  done
done

# The codes the other sources write, each with its places; the lines of their inline test modules
# go to $inline.
declare -A places=()
codes=()
for f in "${sources[@]}"; do
  [[ -v test_file["$f"] ]] && continue
  mapfile -t text < "$f"
  in_test=0
  for ((i = 0; i < ${#text[@]}; i++)); do
    line="${text[i]}"
    if ((in_test)); then
      printf '%s\n' "$line" >> "$inline"
      [[ "$line" == "}" ]] && in_test=0
      continue
    fi
    if [[ "$line" =~ $CFG_TEST && "${text[i + 1]:-}" =~ $INLINE_MODULE ]]; then
      in_test=1
      continue
    fi
    [[ "$line" =~ $CALL ]] || continue
    rest="${line#*"${BASH_REMATCH[0]}"}"
    at=$((i + 1))
    if [[ -z "${rest//[[:space:]]/}" ]]; then
      rest="${text[i + 1]:-}"
      at=$((i + 2))
    fi
    [[ "$rest" =~ $CODE ]] || continue
    code="${BASH_REMATCH[1]}"
    [[ -v places["$code"] ]] || codes+=("$code")
    places["$code"]+="${places[$code]:+, }$f:$at"
  done
done

if ((${#codes[@]} == 0)); then
  echo "$NAME: no code is written as a string literal into Diagnostic::warning or" \
    "Diagnostic::error under crates/*/src — nothing to report."
  exit 0
fi

# Every file of the tests, $inline first, which always exists, so that grep never reads its input.
tests=("$inline" "${!test_file[@]}")
for f in crates/*/tests/**; do [[ -f "$f" ]] && tests+=("$f"); done
untested=()
for code in "${codes[@]}"; do
  grep -qE -e "(^|[^a-z0-9-])$code([^a-z0-9-]|$)" -- "${tests[@]}" && continue
  untested+=("  - $code — ${places[$code]}")
done

if ((${#untested[@]} == 0)); then
  echo "$NAME: a test names each of the ${#codes[@]} code(s) written into Diagnostic::warning" \
    "or Diagnostic::error under crates/*/src — nothing to report."
  exit 0
fi
echo "$NAME: ${#untested[@]} of ${#codes[@]} code(s) written into Diagnostic::warning or" \
  "Diagnostic::error under crates/*/src that no test names — each needs a test that pushes it" \
  "through the load or render that does, or its reason in the pull request:"
printf '%s\n' "${untested[@]}"
exit 0
