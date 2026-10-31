#!/usr/bin/env bash
#
# Self-test of scripts/fuzz.sh, without the nightly toolchain or cargo-fuzz.
#
# Each case copies the script into a throwaway directory with a fixture fuzz/Cargo.toml and the
# seed directories the case asks for, and runs it with stand-ins first on PATH: 'rustup' records
# its arguments, and 'cargo' hands 'metadata' to the real cargo, so that the targets are read as
# the script reads them, answers 'fuzz --version' with the pinned version, and records every other
# call. The case asserts the exit code, a line of the output, and the calls that would have
# installed the toolchain and fuzzed; a target without seeds fails before any of them is made.
#
# Usage:
#     scripts/test-fuzz.sh
# Exit code 0 when every case passes, 1 otherwise.

set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCRIPT="$ROOT/scripts/fuzz.sh"
REAL_CARGO="$(command -v cargo)" || { echo "test-fuzz: cargo is not on PATH" >&2; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failed=0
passed=0

# The stand-ins: every call but 'cargo metadata' and 'cargo fuzz --version' appends its command
# and arguments as one line to $FAKE_LOG.
mkdir -p "$work/bin"
cat > "$work/bin/cargo" << EOF
#!/usr/bin/env bash
if [[ "\${1:-}" == "metadata" ]]; then exec "$REAL_CARGO" "\$@"; fi
if [[ "\$*" == "fuzz --version" ]]; then echo "cargo-fuzz 0.13.2"; exit 0; fi
echo "cargo \$*" >> "\$FAKE_LOG"
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
# the output must contain the needle, and the recorded calls, one per line in the order cargo
# lists the targets, must equal the given ones exactly.
expect() {
  local want="$1" name="$2" dir="$3" needle="$4" calls="$5"
  shift 5
  local out rc log="$work/$name.log"
  [[ -n "$dir" && -f "$dir/scripts/fuzz.sh" ]] || { echo "expect: '$dir' is no fixture" >&2; exit 1; }
  : > "$log"
  out="$(cd "$dir" && PATH="$work/bin:$PATH" FAKE_LOG="$log" bash scripts/fuzz.sh "$@" 2>&1)"; rc=$?
  if [[ $rc -eq "$want" && "$out" == *"$needle"* && "$(cat "$log")" == "$calls" ]]; then
    passed=$((passed + 1))
    return
  fi
  failed=$((failed + 1))
  echo "FAIL: $name (expected exit $want with \"$needle\", got exit $rc)" >&2
  while IFS= read -r l; do printf '    %s\n' "$l"; done <<< "$out" >&2
  echo "    calls expected: $calls" >&2
  while IFS= read -r l; do printf '    calls got:      %s\n' "$l"; done < "$log" >&2
}

BIN_ONE=$'[[bin]]\nname = "one"\npath = "fuzz_targets/one.rs"'
BIN_PATH=$'[[bin]]\nname = "declared"\npath = "elsewhere/odd_name.rs"'
SETUP="rustup toolchain install nightly-2026-10-20 --profile minimal
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
expect 101 "a manifest cargo cannot read stops the run with cargo's message" "$d" \
  "failed to parse manifest" ""

# --- the run ---------------------------------------------------------------------------------

d="$(fixture declared-path "$BIN_PATH" declared)"
expect 0 "a bin declared with a path is fuzzed by its name" "$d" "--> fuzz declared for 300 s" \
  "$SETUP
cargo +nightly-2026-10-20 fuzz run declared fuzz/corpus/declared fuzz/seeds/declared -- -max_total_time=300 $LIMITS"

d="$(fixture both-seeded "$BIN_ONE"$'\n\n'"$BIN_PATH" one declared)"
expect 0 "every seeded bin is fuzzed for the seconds given" "$d" "--> fuzz one for 7 s" \
  "$SETUP
cargo +nightly-2026-10-20 fuzz run declared fuzz/corpus/declared fuzz/seeds/declared -- -max_total_time=7 $LIMITS
cargo +nightly-2026-10-20 fuzz run one fuzz/corpus/one fuzz/seeds/one -- -max_total_time=7 $LIMITS" \
  7

# --- summary ---------------------------------------------------------------------------------

if ((failed)); then
  echo "Fuzz script self-test FAILED ($failed of $((passed + failed)) cases)." >&2
  exit 1
fi
echo "Fuzz script self-test passed ($passed cases)."
exit 0
