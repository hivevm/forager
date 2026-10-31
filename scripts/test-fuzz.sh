#!/usr/bin/env bash
#
# Self-test of scripts/fuzz.sh, without the nightly toolchain or cargo-fuzz.
#
# Each case copies the script into a throwaway directory with a fixture fuzz/Cargo.toml and the
# seed directories the case asks for, and runs it with stand-ins first on PATH: 'rustup' records
# its arguments, and 'cargo' hands 'metadata' to the real cargo, so that the targets are read as
# the script reads them, answers 'fuzz --version' with the pinned version, and records every other
# call, failing the 'fuzz run' of each target named in $FAKE_FAIL. The case asserts the exit code,
# a line of the output, and the calls that would have installed the toolchain and fuzzed; a target
# without seeds fails before any of them is made, and a target that fails does not stop the others.
# The pins, the nightly toolchain and the version of cargo-fuzz, are read from scripts/fuzz.sh, so
# that a bump there needs no edit here.
# Each fixture carries the repository's rust-toolchain.toml, so that cargo reads the manifests
# with the pinned toolchain (ADR-0008), which scripts/install-tools.sh installs.
#
# Usage:
#     scripts/test-fuzz.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/fuzz.sh"
REAL_CARGO="$(command -v cargo)" || { echo "test-fuzz: cargo is not on PATH" >&2; exit 1; }
NIGHTLY="$(sed -n 's/^NIGHTLY="\([^"]*\)"$/\1/p' "$SCRIPT")"
CARGO_FUZZ_VERSION="$(sed -n 's/^CARGO_FUZZ_VERSION="\([^"]*\)"$/\1/p' "$SCRIPT")"
if [[ -z "$NIGHTLY" || -z "$CARGO_FUZZ_VERSION" ]]; then
  echo "test-fuzz: scripts/fuzz.sh sets no NIGHTLY or CARGO_FUZZ_VERSION pin" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# The stand-ins: every call but 'cargo metadata' and 'cargo fuzz --version' appends its command
# and arguments as one line to $FAKE_LOG; 'cargo fuzz run' of a target named in $FAKE_FAIL then
# fails, as a crash would.
mkdir -p "$work/bin"
cat > "$work/bin/cargo" << EOF
#!/usr/bin/env bash
if [[ "\${1:-}" == "metadata" ]]; then exec "$REAL_CARGO" "\$@"; fi
if [[ "\$*" == "fuzz --version" ]]; then echo "cargo-fuzz $CARGO_FUZZ_VERSION"; exit 0; fi
echo "cargo \$*" >> "\$FAKE_LOG"
if [[ "\${2:-} \${3:-}" == "fuzz run" && " \${FAKE_FAIL:-} " == *" \$4 "* ]]; then
  echo "stand-in crash in \$4" >&2
  exit 77
fi
EOF
cat > "$work/bin/rustup" << 'EOF'
#!/usr/bin/env bash
echo "rustup $*" >> "$FAKE_LOG"
EOF
chmod +x "$work/bin/cargo" "$work/bin/rustup"

# fixture <name> <manifest body> [seeded target...] — a directory with the script, a fuzz
# package whose manifest ends in the given body, a lib and the sources the bins name, and a seed
# directory for each seeded target, printed as its path.
fixture() {
  local dir="$work/$1" body="$2"
  shift 2
  mkdir -p "$dir/scripts" "$dir/fuzz/src" "$dir/fuzz/fuzz_targets" "$dir/fuzz/elsewhere"
  cp "$SCRIPT" "$dir/scripts/fuzz.sh"
  cp "$ROOT/rust-toolchain.toml" "$dir/rust-toolchain.toml"
  printf '[package]\nname = "fixture-fuzz"\nversion = "0.0.0"\nedition = "2021"\n' \
    > "$dir/fuzz/Cargo.toml"
  printf 'publish = false\n\n[workspace]\n\n%s\n' "$body" >> "$dir/fuzz/Cargo.toml"
  : > "$dir/fuzz/src/lib.rs"
  printf 'fn main() {}\n' > "$dir/fuzz/fuzz_targets/one.rs"
  printf 'fn main() {}\n' > "$dir/fuzz/elsewhere/odd_name.rs"
  local target
  for target in "$@"; do
    mkdir -p "$dir/fuzz/seeds/$target"
  done
  echo "$dir"
}

# expect <exit code> <name> <dir> <needle> <calls> [args...] — run the script in the directory;
# the exit code must be the given one, or any but 0 when that is 'nonzero'; the output must
# contain the needle, and not $REJECT when that is set; and the recorded calls, one per line, must
# equal the given ones once both are sorted, since cargo, not the manifest, decides the order of
# the targets.
expect() {
  local want="$1" name="$2" dir="$3" needle="$4" calls="$5"
  shift 5
  local out rc log="$work/$name.log"
  [[ -n "$dir" && -f "$dir/scripts/fuzz.sh" ]] || { echo "expect: '$dir' is no fixture" >&2; exit 1; }
  : > "$log"
  out="$(cd "$dir" && PATH="$work/bin:$PATH" FAKE_LOG="$log" bash scripts/fuzz.sh "$@" 2>&1)"; rc=$?
  local rc_ok=0
  if [[ "$want" == nonzero ]]; then ((rc != 0)) && rc_ok=1; else ((rc == want)) && rc_ok=1; fi
  if ((rc_ok)) && [[ "$out" == *"$needle"* ]] \
    && [[ "$(sort "$log")" == "$(if [[ -n "$calls" ]]; then sort <<< "$calls"; fi)" ]] \
    && [[ -z "${REJECT:-}" || "$out" != *"$REJECT"* ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $want with \"$needle\"${REJECT:+ and without \"$REJECT\"}, got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
  echo "    calls expected: $calls" >&2
  while IFS= read -r l; do printf '    calls got:      %s\n' "$l"; done < "$log" >&2
}

BIN_ONE=$'[[bin]]\nname = "one"\npath = "fuzz_targets/one.rs"'
BIN_PATH=$'[[bin]]\nname = "declared"\npath = "elsewhere/odd_name.rs"'
SETUP="rustup toolchain install $NIGHTLY --profile minimal
cargo deny --manifest-path fuzz/Cargo.toml --config deny.toml --locked check advisories sources"
LIMITS="-timeout=10 -rss_limit_mb=2048"

# --- refusals before anything is fuzzed ------------------------------------------------------

d="$(fixture no-bins "")"
expect 1 "a manifest without a bin fails" "$d" "fuzz/Cargo.toml names no [[bin]] target" ""

d="$(fixture no-seeds "$BIN_ONE")"
expect 1 "a bin without seeds fails naming it, before anything is fuzzed" "$d" \
  "the fuzz target one has no seeds under fuzz/seeds/one" ""

d="$(fixture one-of-two-seeded "$BIN_ONE"$'\n\n'"$BIN_PATH" one)"
expect 1 "a second bin without seeds fails naming it" "$d" \
  "the fuzz target declared has no seeds under fuzz/seeds/declared" ""

d="$(fixture auto-bin "")"
mkdir -p "$d/fuzz/src/bin"
printf 'fn main() {}\n' > "$d/fuzz/src/bin/discovered.rs"
expect 1 "a bin cargo discovers without a [[bin]] entry is a target too" "$d" \
  "the fuzz target discovered has no seeds under fuzz/seeds/discovered" ""

d="$(fixture broken-manifest "[[bin]]")"
# A failing cargo stops the run itself, not the empty list of targets it would leave behind.
REJECT="names no [[bin]] target" expect nonzero "a manifest cargo cannot read stops the run" \
  "$d" "" ""

# --- the run ---------------------------------------------------------------------------------

d="$(fixture declared-path "$BIN_PATH" declared)"
expect 0 "a bin declared with a path is fuzzed by its name" "$d" "--> fuzz declared for 300 s" \
  "$SETUP
cargo +$NIGHTLY fuzz run declared fuzz/corpus/declared fuzz/seeds/declared -- -max_total_time=300 $LIMITS"

d="$(fixture both-seeded "$BIN_ONE"$'\n\n'"$BIN_PATH" one declared)"
expect 0 "every seeded bin is fuzzed for the seconds given" "$d" "--> fuzz one for 7 s" \
  "$SETUP
cargo +$NIGHTLY fuzz run declared fuzz/corpus/declared fuzz/seeds/declared -- -max_total_time=7 $LIMITS
cargo +$NIGHTLY fuzz run one fuzz/corpus/one fuzz/seeds/one -- -max_total_time=7 $LIMITS" \
  7

# Each of the two targets fails in turn, so that whichever cargo runs first, a case fails it and
# asserts that the other is fuzzed too and is not named.
both_runs="$SETUP
cargo +$NIGHTLY fuzz run declared fuzz/corpus/declared fuzz/seeds/declared -- -max_total_time=300 $LIMITS
cargo +$NIGHTLY fuzz run one fuzz/corpus/one fuzz/seeds/one -- -max_total_time=300 $LIMITS"
for bad in one declared; do
  good=declared
  [[ "$bad" == declared ]] && good=one
  d="$(fixture "fails-$bad" "$BIN_ONE"$'\n\n'"$BIN_PATH" one declared)"
  FAKE_FAIL="$bad" REJECT="the fuzz target $good failed" expect 1 \
    "a failing $bad does not stop the other target, and the run fails naming it" "$d" \
    "the fuzz target $bad failed" "$both_runs"
done

d="$(fixture fails-both "$BIN_ONE"$'\n\n'"$BIN_PATH" one declared)"
FAKE_FAIL="one declared" expect 1 "the run names every target that failed" "$d" \
  "the fuzz target declared failed" "$both_runs"
FAKE_FAIL="one declared" expect 1 "the run names every target that failed, the other too" "$d" \
  "the fuzz target one failed" "$both_runs"

# --- the workflow ----------------------------------------------------------------------------

# The run fails only after every target has run, so the workflow uploads the inputs of every
# target that failed when it uploads fuzz/artifacts/ as a whole, after a failure.
upload="$(sed -n '/- name: Keep the inputs that failed/,/^[[:space:]]*- /p' "$ROOT/.github/workflows/fuzz.yml")"
if [[ "$upload" == *"if: failure()"* && "$upload" == *"actions/upload-artifact@"* ]] \
  && grep -qE '^[[:space:]]*path: fuzz/artifacts/?$' <<< "$upload"; then
  passed=$((passed + 1))
else
  failed=$((failed + 1))
  echo "FAIL: the fuzz workflow uploads fuzz/artifacts/ after a failure" >&2
fi

# --- summary ---------------------------------------------------------------------------------

if ((failed)); then
  echo "Fuzz script self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Fuzz script self-test passed ($passed cases)."
exit 0
