#!/usr/bin/env bash
#
# Self-test of scripts/check-guards.sh. Verifies: ARCH-third-party-failures-guarded
#
# Each case writes a fixture with a Cargo.lock and one Rust file under crates/, runs the check
# against it, and asserts the exit code and the line that names the marker or the problem.
#
# Usage:
#     scripts/test-check-guards.sh
# Exit code 0 when every case passes, 1 otherwise.
# Inputs: scripts/check-guards.sh scripts/lib/

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"

CHECK="$ROOT/scripts/check-guards.sh"

# The lock of the fixtures: agg-rust at one version, i_overlay at two, as a workspace that takes
# one directly and another through a dependency holds it.
LOCK='version = 4

[[package]]
name = "agg-rust"
version = "1.2.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "i_overlay"
version = "4.5.2"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "i_overlay"
version = "9.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = [
 "agg-rust",
]'

# repo <name> <text> — a fixture with the lock above and one file, crates/x/src/lib.rs, that
# holds the text, printed as its path.
repo() {
  local dir="$work/$1"
  mkdir -p "$dir/crates/x/src"
  printf '%s\n' "$LOCK" > "$dir/Cargo.lock"
  printf '%s\n' "$2" > "$dir/crates/x/src/lib.rs"
  echo "$dir"
}

# expect_check <name> <dir> <code> <needle> — run the check; it must exit with the code and its
# output contain the needle (expect, scripts/lib/expect.sh).
expect_check() { expect "$1" "$3" "$4" bash "$CHECK" "$2"; }

guard() { printf '// Guard: %s %s\nfn guard() {}' "$1" "$2"; }

dir="$(repo locked "$(guard agg-rust 1.2.0)")"
expect_check "a marker of the locked version passes" "$dir" 0 "Guard check passed (1 marker)."
expect_check "a passing run lists the marker" "$dir" 0 "crates/x/src/lib.rs:1: agg-rust 1.2.0"

dir="$(repo other "$(guard agg-rust 1.1.0)")"
expect_check "a marker of another version fails, naming the file, the crate, and both versions" \
  "$dir" 1 "crates/x/src/lib.rs:1 names agg-rust 1.1.0, but Cargo.lock holds agg-rust 1.2.0"

dir="$(repo second "$(guard i_overlay 4.5.2)")"
expect_check "a crate held at two versions matches the one the marker names" \
  "$dir" 0 "Guard check passed (1 marker)."
dir="$(repo first "$(guard i_overlay 9.1.0)")"
expect_check "a crate held at two versions matches the other one too" \
  "$dir" 0 "Guard check passed (1 marker)."
dir="$(repo neither "$(guard i_overlay 5.0.0)")"
expect_check "a crate held at two versions fails where neither is the marker's" \
  "$dir" 1 "names i_overlay 5.0.0, but Cargo.lock holds i_overlay 4.5.2 9.1.0"

dir="$(repo unheld "$(guard geo 0.33.1)")"
expect_check "a marker of a crate the lock does not hold fails" \
  "$dir" 1 "crates/x/src/lib.rs:1 names geo 0.33.1, which Cargo.lock does not hold"

dir="$(repo prefix "$(guard agg 1.2.0)")"
expect_check "a crate whose name begins another's is not that one" \
  "$dir" 1 "names agg 1.2.0, which Cargo.lock does not hold"

dir="$(repo malformed '// Guard: agg-rust
fn guard() {}')"
expect_check "a marker without a version fails" "$dir" 1 \
  "crates/x/src/lib.rs:1 holds a guard marker that is not '// Guard: <crate> <version>'"

dir="$(repo indented "$(printf 'mod m {\n    // Guard: agg-rust 1.2.0\n    fn guard() {}\n}')")"
expect_check "an indented marker is read" "$dir" 0 "crates/x/src/lib.rs:2: agg-rust 1.2.0"

dir="$(repo two "$(guard agg-rust 1.2.0)
$(guard i_overlay 1.0.0)")"
expect_check "each marker is held to the lock" "$dir" 1 "names i_overlay 1.0.0"

# shellcheck disable=SC2016 # the backticks are the prose's, not an expansion
dir="$(repo prose '//! A guard is marked by a line such as `// Guard: <crate> <version>`.
fn f() {}')"
expect_check "a mention in prose is no marker" "$dir" 0 "Guard check passed (0 markers)."

dir="$(repo built "$(guard agg-rust 1.2.0)")"
mkdir -p "$dir/crates/x/target/debug"
printf '// Guard: agg-rust 0.0.1\n' > "$dir/crates/x/target/debug/out.rs"
expect_check "a file under target/ is not read" "$dir" 0 "Guard check passed (1 marker)."

dir="$(repo nolock "$(guard agg-rust 1.2.0)")"
rm "$dir/Cargo.lock"
expect_check "a tree without Cargo.lock fails" "$dir" 1 "no Cargo.lock"

expect_check "a root that is no directory is refused" "$work/absent" 2 "is not a directory"

summary "Guard check self-test"
