#!/usr/bin/env bash
#
# Self-test of scripts/package.sh: its check, its packing, and its build path.
#
# - The check: each case writes a fixture archive, as the script packs one, with or without the
#   files every artefact carries, runs `scripts/package.sh --check` on it, and asserts the exit
#   code and the line that names the problem.
# - The packing: `pack` of scripts/lib/package.sh writes an archive the check passes.
# - The sourced check: `check_archives`, sourced into a shell, returns its status, and the shell
#   goes on.
# - The build path: `scripts/package.sh` without arguments runs on a fixture tree, with cargo and
#   rustc stubbed on PATH and scripts/build-web.sh stubbed in the tree, since the real build takes
#   minutes. The cases assert both archives and what they hold, the release build's arguments,
#   and that a stale file of target/web/ is not packed.
#
# Usage:
#     scripts/test-package.sh
# Inputs: scripts/package.sh scripts/lib/ THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE
# Exit code 0 when every case passes, 1 otherwise.

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

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

# expect_check <name> <code> <needle> <archive>... — run the check; it must exit with the code
# and its output contain the needle (expect, scripts/lib/expect.sh).
expect_check() {
  local name="$1" code="$2" needle="$3"; shift 3
  expect "$name" "$code" "$needle" bash "$SCRIPT" --check "$@"
}

full=(THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE)

expect_check "an archive that carries the notices and both licences passes" 0 \
  "Package check passed (1 artefact(s)" "$(archive forager-0.1.0-x86_64 "${full[@]}")"
expect_check "an archive without THIRD-PARTY-NOTICES.md fails, by name" 1 \
  "forager-web-0.1.0.tar.gz does not carry forager-web-0.1.0/THIRD-PARTY-NOTICES.md" \
  "$(archive forager-web-0.1.0 LICENSE LICENSE-APACHE)"
expect_check "an archive without LICENSE-APACHE fails, by name" 1 \
  "no-apache.tar.gz does not carry no-apache/LICENSE-APACHE" \
  "$(archive no-apache THIRD-PARTY-NOTICES.md LICENSE)"
expect_check "an archive without LICENSE fails" 1 "does not carry no-mit/LICENSE" \
  "$(archive no-mit THIRD-PARTY-NOTICES.md LICENSE-APACHE)"
without_franklin="$(grep -vF 'Wm. Randolph Franklin' "$ROOT/THIRD-PARTY-NOTICES.md")"
expect_check "a copy of the notices without Franklin's copyright line fails, by name" 1 \
  "no-franklin.tar.gz carries a THIRD-PARTY-NOTICES.md without" \
  "$(archive no-franklin LICENSE LICENSE-APACHE "THIRD-PARTY-NOTICES.md=$without_franklin")"
without_mapserver="$(grep -vF 'Regents of the University of Minnesota' "$ROOT/THIRD-PARTY-NOTICES.md")"
expect_check "a copy of the notices without MapServer's copyright line fails" 1 \
  "carries a THIRD-PARTY-NOTICES.md without" \
  "$(archive no-mapserver LICENSE LICENSE-APACHE "THIRD-PARTY-NOTICES.md=$without_mapserver")"

# The notices beside the directory, not in it, are not carried by it.
loose="$work/loose.d"
mkdir -p "$loose/loose"
cp "$ROOT/THIRD-PARTY-NOTICES.md" "$ROOT/LICENSE" "$ROOT/LICENSE-APACHE" "$loose/"
tar -czf "$work/loose.tar.gz" -C "$loose" loose THIRD-PARTY-NOTICES.md LICENSE LICENSE-APACHE
expect_check "notices outside the archive's directory are not carried" 1 \
  "loose.tar.gz does not carry loose/THIRD-PARTY-NOTICES.md" "$work/loose.tar.gz"

printf 'not an archive\n' > "$work/broken.tar.gz"
expect_check "a file that is no archive fails, by name" 1 "broken.tar.gz is no gzipped tar archive" \
  "$work/broken.tar.gz"
expect_check "every archive is checked, and the count names those that fail" 1 \
  "Package check FAILED (1 of 2 artefact(s)" \
  "$work/forager-0.1.0-x86_64.tar.gz" "$work/no-apache.tar.gz"
expect_check "--check without an archive is a usage error" 2 "needs at least one archive"
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
expect_check "an archive the script packs carries the notices" 0 "Package check passed (1 artefact(s)" \
  "$packed/forager-web-9.9.9.tar.gz"

# check_archives, sourced into a shell, returns its status: a missing file fails the check, named,
# and the shell that sourced it goes on.
sourced_archive="$(archive sourced-no-apache THIRD-PARTY-NOTICES.md LICENSE)"
sourced="$(bash -c 'ROOT="$1"; source "$1/scripts/lib/notice.sh"; source "$1/scripts/lib/package.sh"
  check_archives "$2"; echo "went on with $?"' _ "$ROOT" "$sourced_archive" 2>&1)"
held=0
[[ "$sourced" == *"does not carry sourced-no-apache/LICENSE-APACHE"* \
  && "$sourced" == *"went on with 1" ]] || held=1
check "check_archives, sourced, returns non-zero on a missing file and the shell goes on" \
  test "$held" -eq 0

# The build path, run on a fixture tree that holds the script, its libraries, and the notices,
# with cargo and rustc stubbed on PATH and scripts/build-web.sh stubbed in the tree: each stub
# writes what the real one would, and records how it was called.
tree="$work/tree"
mkdir -p "$tree/scripts/lib" "$tree/stubs" "$tree/target/web"
cp "$ROOT/scripts/package.sh" "$tree/scripts/"
cp "$ROOT/scripts/lib/notice.sh" "$ROOT/scripts/lib/package.sh" "$tree/scripts/lib/"
cp "$ROOT/THIRD-PARTY-NOTICES.md" "$ROOT/LICENSE" "$ROOT/LICENSE-APACHE" "$tree/"
printf '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "9.9.9"\nedition = "2024"\n' \
  > "$tree/Cargo.toml"
cat > "$tree/stubs/rustc" <<'STUB'
#!/usr/bin/env bash
printf 'rustc 1.0.0\nhost: stub-host\n'
STUB
cat > "$tree/stubs/cargo" <<'STUB'
#!/usr/bin/env bash
echo "cargo $*" >> "$STUB_LOG"
dir=target
while (($#)); do [[ "$1" == --target-dir ]] && dir="$2"; shift; done
mkdir -p "$dir/release" && printf 'binary\n' > "$dir/release/forager"
STUB
cat > "$tree/scripts/build-web.sh" <<'STUB'
#!/usr/bin/env bash
echo "build-web in $PWD" >> "$STUB_LOG"
[[ -e target/web/stale.js ]] && echo "build-web found stale.js" >> "$STUB_LOG"
mkdir -p target/web
printf 'wasm\n' > target/web/forager_bindings_wasm_bg.wasm
printf 'glue\n' > target/web/forager_bindings_wasm.js
STUB
chmod +x "$tree/stubs/rustc" "$tree/stubs/cargo"
# A file an earlier build left in target/web/, which no build of this one writes.
printf 'stale\n' > "$tree/target/web/stale.js"
built="$(cd "$work" && STUB_LOG="$tree/stubs.log" PATH="$tree/stubs:$PATH" \
  bash "$tree/scripts/package.sh" 2>&1)"
built_rc=$?
cli_archive="$tree/target/package/forager-9.9.9-stub-host.tar.gz"
web_archive="$tree/target/package/forager-web-9.9.9.tar.gz"
cli_listing="$(tar -tzf "$cli_archive" 2>/dev/null)"
web_listing="$(tar -tzf "$web_archive" 2>/dev/null)"
log="$(cat "$tree/stubs.log" 2>/dev/null)"
held=0
((built_rc == 0)) && [[ "$built" == *"Package check passed (2 artefact(s)"* ]] || held=1
check "the build, without arguments, writes both archives and the check passes over them" \
  test "$held" -eq 0
check "the CLI's archive holds the binary the release build writes, beside the notices" \
  grep -qxF forager-9.9.9-stub-host/forager <<< "$cli_listing"
check "the browser bindings' package holds the wasm build-web.sh writes" \
  grep -qxF forager-web-9.9.9/forager_bindings_wasm_bg.wasm <<< "$web_listing"
check "the browser bindings' package holds the JavaScript glue build-web.sh writes" \
  grep -qxF forager-web-9.9.9/forager_bindings_wasm.js <<< "$web_listing"
check "the build is the locked release build of the CLI, into the tree's target directory" \
  grep -qxF "cargo build --locked --release --target-dir $tree/target -p forager-cli" <<< "$log"
check "build-web.sh runs at the root of the tree" grep -qxF "build-web in $tree" <<< "$log"
held=0
[[ "$log" != *"found stale.js"* ]] || held=1
check "build-web.sh finds no file an earlier build left in target/web/" test "$held" -eq 0
held=0
[[ -n "$web_listing" && "$web_listing" != *stale.js* ]] || held=1
check "the browser bindings' package does not hold a stale file of target/web/" test "$held" -eq 0
if ((built_rc != 0)); then
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$built" >&2
fi

summary "Package check self-test"
