#!/usr/bin/env bash
#
# Self-test of scripts/sensor-diagnostics-untested.sh. Verifies: ADR-0007
#
# Each case writes a throwaway tree of crates under a temporary directory, a crate's source that
# pushes a diagnostic and the tests beside it, runs the sensor on it, and asserts the exit code and
# the line a reader would meet. A case either names a code no test names, with its place, or holds
# the sensor silent on a code a test names: in a crate's tests/, in an inline #[cfg(test)] module,
# in a module file a #[cfg(test)] declaration names, or in another crate's tests. Further cases
# hold what is no code or no test: a code passed as a variable, a code a test module writes, a
# test that names a longer code, the source after a test module, and a root that does not exist.
#
# Inputs: scripts/sensor-diagnostics-untested.sh scripts/lib/
#
# Usage:
#     scripts/test-sensor-diagnostics-untested.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

# shellcheck source-path=SCRIPTDIR source=lib/selftest.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/selftest.sh"
# shellcheck source-path=SCRIPTDIR source=lib/expect.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib/expect.sh"
SENSOR="$ROOT/scripts/sensor-diagnostics-untested.sh"

# tree <name> — an empty directory for a tree of crates, printed as its path.
tree() {
  mkdir -p "$work/$1/crates"
  echo "$work/$1"
}

# put <dir> <path> <content> — write a file of the tree.
put() {
  mkdir -p "$(dirname "$1/$2")"
  printf '%s\n' "$3" > "$1/$2"
}

# expect_sensor <exit code> <name> <dir> <needle> — run the sensor on the tree; its output must
# contain the needle, and not $REJECT when that is set (expect, scripts/lib/expect.sh).
expect_sensor() {
  expect "$2" "$1" "$4" bash "$SENSOR" --root "$3"
}

# A source that pushes the warning "layer-not-drawn", its code on the line after the call, as
# rustfmt wraps a long call.
PUSH_WRAPPED='pub fn draw(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::warning(
        "layer-not-drawn",
        "the layer is not drawn",
        None,
    ));
}'
# A source that pushes the error "size-missing", its code on the line of the call.
PUSH_INLINE='pub fn size(diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::error("size-missing", "the map has no SIZE", None));
}'
NOTHING_NAMED='Diagnostics untested sensor: a test names each of the'

# --- cases -----------------------------------------------------------------------------------

d="$(tree untested)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_WRAPPED"
put "$d" "crates/engine/src/size.rs" "$PUSH_INLINE"
put "$d" "crates/engine/tests/size.rs" 'assert!(codes.contains(&"size-missing"));'
expect_sensor 0 "a code no test names is named with its file and line, and the sensor exits 0" \
  "$d" "  - layer-not-drawn — crates/engine/src/lib.rs:3"
REJECT="size-missing" expect_sensor 0 "a code a crate's tests name is not named" "$d" \
  "Diagnostics untested sensor: 1 of 2 code(s)"

d="$(tree tested)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_WRAPPED"
put "$d" "crates/engine/src/size.rs" "$PUSH_INLINE"
put "$d" "crates/engine/tests/render.rs" 'assert_eq!(diagnostics[0].code, "layer-not-drawn");'
put "$d" "crates/engine/tests/size.rs" 'assert!(codes.contains(&"size-missing"));'
expect_sensor 0 "codes a crate's tests name leave nothing to report" "$d" \
  "$NOTHING_NAMED 2 code(s)"

d="$(tree inline-module)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_WRAPPED

#[cfg(test)]
mod tests {
    #[test]
    fn a_layer_without_data_is_not_drawn() {
        assert_eq!(super::codes(), [\"layer-not-drawn\"]);
    }
}"
expect_sensor 0 "a code an inline #[cfg(test)] module names leaves nothing to report" "$d" \
  "$NOTHING_NAMED 1 code(s)"

d="$(tree module-file)"
put "$d" "crates/engine/src/lib.rs" "mod size;
#[cfg(test)]
mod tests;"
put "$d" "crates/engine/src/tests.rs" 'assert!(super::size::codes().is_empty());'
put "$d" "crates/engine/src/size.rs" "$PUSH_INLINE
#[cfg(test)]
mod tests;"
put "$d" "crates/engine/src/size/tests.rs" 'assert_eq!(codes(), ["size-missing"]);'
expect_sensor 0 "a code the file of a #[cfg(test)] module names leaves nothing to report" "$d" \
  "$NOTHING_NAMED 1 code(s)"

d="$(tree other-crate)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_WRAPPED"
put "$d" "crates/cli/tests/render.rs" 'assert!(stderr.contains("layer-not-drawn"));'
expect_sensor 0 "a code the tests of another crate name leaves nothing to report" "$d" \
  "$NOTHING_NAMED 1 code(s)"

d="$(tree variable)"
put "$d" "crates/engine/src/lib.rs" 'fn warn(code: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic::warning(code, "a message", None));
}'
expect_sensor 0 "a code passed as a variable is no code the sensor reads" "$d" \
  "Diagnostics untested sensor: no code is written as a string literal"

d="$(tree test-writes)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_INLINE

#[cfg(test)]
mod tests {
    fn fixture() -> Diagnostic {
        Diagnostic::warning(\"fixture-only\", \"made by a test\", None)
    }
}"
REJECT="fixture-only" expect_sensor 0 "a code a test module writes is no code of the source" \
  "$d" "  - size-missing — crates/engine/src/lib.rs:2"

d="$(tree longer-code)"
put "$d" "crates/engine/src/lib.rs" "$PUSH_WRAPPED"
put "$d" "crates/engine/tests/render.rs" 'assert!(codes.contains(&"layer-not-drawn-twice"));
assert!(codes.contains(&"no-layer-not-drawn"));'
expect_sensor 0 "a test that names a longer code does not name the code" "$d" \
  "  - layer-not-drawn — crates/engine/src/lib.rs:3"

d="$(tree after-module)"
put "$d" "crates/ports/src/lib.rs" "#[cfg(test)]
mod tests {
    #[test]
    fn nothing() {}
}

$PUSH_INLINE

pub fn named() -> &'static str {
    \"size-missing\"
}"
expect_sensor 0 "the source after a test module is no test" "$d" \
  "  - size-missing — crates/ports/src/lib.rs:8"

expect_sensor 2 "a root that does not exist fails" "$work/no-such-root" "is not a directory"

# --- summary ---------------------------------------------------------------------------------

summary "Diagnostics untested sensor self-test"
