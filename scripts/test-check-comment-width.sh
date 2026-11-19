#!/usr/bin/env bash
#
# Self-test of scripts/check-comment-width.sh.
#
# Each case writes a fixture git repository with one Rust file, runs the check against it, and
# asserts the exit code and the line that names the comment or the result.
#
# Usage:
#     scripts/test-check-comment-width.sh
# Exit code 0 when every case passes, 1 otherwise.
# Inputs: scripts/check-comment-width.sh scripts/lib/

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

CHECK="$ROOT/scripts/check-comment-width.sh"

# repo <name> <text> [path] — a fixture repository with one file, crates/x/src/lib.rs unless a
# path is given, that holds the text, printed as the repository's path. The file is not committed:
# the check reads what git does not ignore.
repo() {
  local dir="$work/$1" path="${3:-crates/x/src/lib.rs}"
  mkdir -p "$dir/$(dirname "$path")"
  git init -q "$dir"
  printf '%s\n' "$2" > "$dir/$path"
  echo "$dir"
}

# line <prefix> <width> [fill] — the prefix padded with the fill character, 'a' by default, to
# the width in characters.
line() {
  local text="$1" fill="${3:-a}"
  while ((${#text} < $2)); do text+="$fill"; done
  printf '%s' "$text"
}

# expect_check <name> <dir> <code> <needle> — run the check; it must exit with the code and its
# output contain the needle (expect, scripts/lib/expect.sh).
expect_check() { expect "$1" "$3" "$4" bash "$CHECK" "$2"; }

export LC_ALL=C.UTF-8

wide="$(line '/// ' 101)"
check "the fixture's doc line is 101 characters" test "${#wide}" -eq 101
dir="$(repo doc "fn f() {}
$wide
fn g() {}")"
expect_check "a /// line of 101 characters fails, naming the file, the line, and its width" \
  "$dir" 1 "crates/x/src/lib.rs:2: 101 characters"

multi="$(line '// a product × a ratio π ' 100 'π')"
check "the fixture's multi-byte comment is 100 characters" test "${#multi}" -eq 100
check "the fixture's multi-byte comment is more than 100 bytes" \
  test "$(printf '%s' "$multi" | wc -c)" -gt 100
literal="$(line '    let s = "' 149 'b')\""
check "the fixture's line of a string literal is 150 characters" test "${#literal}" -eq 150
dir="$(repo fits "$multi
fn f() {
$literal;
}")"
expect_check "a comment of 100 characters, some multi-byte, and a long string literal pass" \
  "$dir" 0 "Comment width check passed (1 file)."

dir="$(repo inner "$(line '//! ' 101)")"
expect_check "a //! line of 101 characters fails" "$dir" 1 "crates/x/src/lib.rs:1: 101 characters"

dir="$(repo indented "mod m {
$(line '    // ' 102)
}")"
expect_check "an indented // line fails with its indentation counted" \
  "$dir" 1 "crates/x/src/lib.rs:2: 102 characters"

dir="$(repo raw "const FIXTURE: &str = r#\"
$(line 'a raw string of a fixture ' 140)
\"#;")"
expect_check "a line of a raw string that is no comment passes" \
  "$dir" 0 "Comment width check passed (1 file)."

dir="$(repo trailing "$(line 'fn f() {} // ' 110)")"
expect_check "a comment after code on its line is not this check's: rustfmt's" \
  "$dir" 0 "Comment width check passed (1 file)."

dir="$(repo two "$(line '// ' 101)
$(line '/// ' 103)")"
expect_check "each wide comment is named" "$dir" 1 "crates/x/src/lib.rs:2: 103 characters"
expect_check "the failure counts the wide comments" "$dir" 1 \
  "Comment width check FAILED (2 comment lines over 100 characters"

dir="$(repo fuzz "$(line '// ' 101)" fuzz/fuzz_targets/render.rs)"
expect_check "a file under fuzz/ is read" "$dir" 1 "fuzz/fuzz_targets/render.rs:1: 101 characters"

dir="$(repo elsewhere "$(line '// ' 101)" tools/x/src/lib.rs)"
expect_check "a file outside crates/ and fuzz/ is not read" \
  "$dir" 0 "Comment width check passed (0 files)."

dir="$(repo other "$(line '// ' 101)" crates/x/build.txt)"
expect_check "a file that is no .rs file is not read" "$dir" 0 "Comment width check passed (0 files)."

dir="$(repo ignored "fn f() {}")"
mkdir -p "$dir/crates/x/target/debug"
printf '%s\n' "$(line '// ' 101)" > "$dir/crates/x/target/debug/out.rs"
printf 'target/\n' > "$dir/.gitignore"
expect_check "a file git ignores is not read" "$dir" 0 "Comment width check passed (1 file)."

dir="$work/nogit"
mkdir -p "$dir/crates"
expect_check "a root that is no git repository fails" "$dir" 1 "cannot list the Rust files"

expect_check "a root that is no directory is refused" "$work/absent" 2 "is not a directory"

summary "Comment width check self-test"
