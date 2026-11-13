#!/usr/bin/env bash
#
# Self-test of the check of scripts/package.sh.
#
# Each case writes a fixture archive, as the script packs one, with or without the files every
# artefact carries, runs `scripts/package.sh --check` on it, and asserts the exit code and the line
# that names the problem. The build itself is not run: it takes minutes, and its archives are
# those `pack` of scripts/lib/package.sh writes, which the last case packs from the repository's
# own files.
#
# Usage:
#     scripts/test-package.sh
# Inputs: scripts/package.sh scripts/lib/ THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE
# Exit code 0 when every case passes, 1 otherwise.

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"

SCRIPT="$ROOT/scripts/package.sh"
# shellcheck source-path=SCRIPTDIR source=lib/notice.sh
source "$ROOT/scripts/lib/notice.sh"
# shellcheck source-path=SCRIPTDIR source=lib/package.sh
source "$ROOT/scripts/lib/package.sh"

# archive <name> <file>... — a fixture archive <name>.tar.gz under $work whose one directory
# <name> holds a binary and each named file, copied from the repository, printed as its path.
# A file given as <path>=<text> holds the text instead.
archive() {
  local name="$1"; shift
  local dir="$work/$name.d" spec
  mkdir -p "$dir/$name"
  printf 'binary\n' > "$dir/$name/forager"
  for spec in "$@"; do
    if [[ "$spec" == *=* ]]; then
      printf '%s\n' "${spec#*=}" > "$dir/$name/${spec%%=*}"
    else
      cp "$ROOT/$spec" "$dir/$name/"
    fi
  done
  tar -czf "$work/$name.tar.gz" -C "$dir" "$name"
  echo "$work/$name.tar.gz"
}

# expect <name> <code> <needle> <archive>... — run the check; it must exit with the code and its
# output contain the needle.
expect() {
  local name="$1" code="$2" needle="$3"; shift 3
  local out rc
  out="$(bash "$SCRIPT" --check "$@" 2>&1)"; rc=$?
  if ((rc == code)) && [[ "$out" == *"$needle"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $code with \"$needle\", exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
}

full=(THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE)

expect "an archive that carries the notices and both licences passes" 0 \
  "Package check passed (1 artefact(s)" "$(archive forager-0.1.0-x86_64 "${full[@]}")"
expect "an archive without THIRD-PARTY-NOTICES.md fails, by name" 1 \
  "forager-web-0.1.0.tar.gz does not carry forager-web-0.1.0/THIRD-PARTY-NOTICES.md" \
  "$(archive forager-web-0.1.0 LICENSE LICENSE-APACHE)"
expect "an archive without LICENSE-APACHE fails, by name" 1 \
  "no-apache.tar.gz does not carry no-apache/LICENSE-APACHE" \
  "$(archive no-apache THIRD-PARTY-NOTICES.md LICENSE)"
expect "an archive without LICENSE fails" 1 "does not carry no-mit/LICENSE" \
  "$(archive no-mit THIRD-PARTY-NOTICES.md LICENSE-APACHE)"
without_franklin="$(grep -vF 'Wm. Randolph Franklin' "$ROOT/THIRD-PARTY-NOTICES.md")"
expect "a copy of the notices without Franklin's copyright line fails, by name" 1 \
  "no-franklin.tar.gz carries a THIRD-PARTY-NOTICES.md without" \
  "$(archive no-franklin LICENSE LICENSE-APACHE "THIRD-PARTY-NOTICES.md=$without_franklin")"
without_mapserver="$(grep -vF 'Regents of the University of Minnesota' "$ROOT/THIRD-PARTY-NOTICES.md")"
expect "a copy of the notices without MapServer's copyright line fails" 1 \
  "carries a THIRD-PARTY-NOTICES.md without" \
  "$(archive no-mapserver LICENSE LICENSE-APACHE "THIRD-PARTY-NOTICES.md=$without_mapserver")"

# The notices beside the directory, not in it, are not carried by it.
loose="$work/loose.d"
mkdir -p "$loose/loose"
cp "$ROOT/THIRD-PARTY-NOTICES.md" "$ROOT/LICENSE" "$ROOT/LICENSE-APACHE" "$loose/"
tar -czf "$work/loose.tar.gz" -C "$loose" loose THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE
expect "notices outside the archive's directory are not carried" 1 \
  "loose.tar.gz does not carry loose/THIRD-PARTY-NOTICES.md" "$work/loose.tar.gz"

printf 'not an archive\n' > "$work/broken.tar.gz"
expect "a file that is no archive fails, by name" 1 "broken.tar.gz is no gzipped tar archive" \
  "$work/broken.tar.gz"
expect "every archive is checked, and the count names those that fail" 1 \
  "Package check FAILED (1 of 2 artefact(s)" \
  "$work/forager-0.1.0-x86_64.tar.gz" "$work/no-apache.tar.gz"
expect "--check without an archive is a usage error" 2 "needs at least one archive"
# Called through check (SC2317 before ShellCheck 0.11, SC2329 from it).
# shellcheck disable=SC2317,SC2329
# unknown_argument_is_a_usage_error — the script, given an argument it does not know, exits 2 and
# names it.
unknown_argument_is_a_usage_error() {
  local out rc
  out="$(bash "$SCRIPT" --bogus 2>&1)"; rc=$?
  ((rc == 2)) && [[ "$out" == *"unknown argument '--bogus'"* ]]
}
check "an unknown argument is a usage error" unknown_argument_is_a_usage_error

# The script's own packing, run on a directory of fixture files, writes an archive the check
# passes: what the build packs carries the repository's notices.
packed="$work/packed"
mkdir -p "$packed/files"
printf 'wasm\n' > "$packed/files/forager_bindings_wasm_bg.wasm"
pack "$packed/files" "$packed/forager-web-9.9.9.tar.gz"
expect "an archive the script packs carries the notices" 0 "Package check passed (1 artefact(s)" \
  "$packed/forager-web-9.9.9.tar.gz"

summary "Package check self-test"
