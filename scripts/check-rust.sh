#!/usr/bin/env bash
# Builds, tests, and lints the Rust workspace (ADR-0008): formatting, clippy with warnings as
# errors, the tests, the style document's JSON Schema against its checked-in file (ADR-0020),
# the load of the msautotest suite at the pinned MapServer release (G-1, fetched once into
# target/ by scripts/fetch-msautotest.sh) and the conformance harness over its cases, the build
# of every crate that must run in the browser for wasm32-unknown-unknown (Q-1), the WebAssembly
# bindings and their size (Q-5) once they exist, and cargo deny (ADR-0012). scripts/check-all.sh
# and the `rust` job of checks.yml run it; the tools come from scripts/install-tools.sh. Two
# fixture workspaces show that cargo deny refuses a non-permissive licence and that clippy bans
# hash maps, clocks, and randomness in the core alone (Q-2).
#
# Usage: bash scripts/check-rust.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Crates that never run in the browser: hosts with a file system, and tooling.
NATIVE_ONLY=(forager-cli forager-dev-server forager-arch-test)
# The gzipped size of the browser bindings may not exceed this (Q-5).
WASM_BUDGET_BYTES=$((3 * 1024 * 1024))

step() { printf '\n--> %s\n' "$1"; }

step "Formatting"
cargo fmt --all --check

step "Clippy"
cargo clippy --workspace --all-targets --locked -- -D warnings

step "Tests"
cargo test --workspace --locked

step "The style document's JSON Schema is the one the IR derives (ADR-0020)"
cargo clippy -p forager-style-ir --features schema --all-targets --locked -- -D warnings
cargo test -p forager-style-ir --features schema --locked --test schema
# The schema stays out of the browser bundle (Q-5): no crate the bindings build enables it.
if cargo tree -p forager-bindings-wasm --locked -e normal | grep -q '^.*schemars '; then
  echo "check-rust: the browser bindings depend on schemars; the schema feature is enabled" >&2
  exit 1
fi
step "msautotest: every mapfile loads (G-1), and the cases in scope hold their outcome"
suite="$(bash scripts/fetch-msautotest.sh)"
# The noise measurement needs MapServer, which only .github/workflows/noise.yml builds (ADR-0027).
FORAGER_MSAUTOTEST="$suite" cargo test -p forager-cli --locked --test msautotest -- --ignored \
  --skip mapserver_renders_the_cases_in_scope_within_its_noise

step "Clippy bans hash maps, clocks, and randomness in the core alone (Q-2)"
# Verifies: Q-2
# A fixture workspace with this clippy.toml and these workspace lints: a crate that carries the line
# of a core crate's lib.rs must fail on every banned use, and one without it, as a host or an
# adapter, must pass.
lints="$(mktemp -d)"
trap 'rm -rf "$lints"' EXIT
mkdir -p "$lints/core/src" "$lints/host/src"
cp clippy.toml rust-toolchain.toml "$lints/"
{
  printf '[workspace]\nmembers = ["core", "host"]\nresolver = "3"\n\n'
  awk '/^\[/ { keep = ($0 ~ /^\[workspace\.lints\./) } keep' Cargo.toml
} > "$lints/Cargo.toml"
for member in core host; do
  printf '[package]\nname = "%s"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n[lints]\nworkspace = true\n' \
    "$member" > "$lints/$member/Cargo.toml"
done
uses='pub fn uses() -> usize {
    let map = std::collections::HashMap::<u8, u8>::new();
    let set = std::collections::HashSet::<u8>::new();
    let _: std::hash::RandomState = std::hash::RandomState::new();
    let _: std::time::SystemTime = std::time::SystemTime::now();
    let _: std::time::Instant = std::time::Instant::now();
    map.len() + set.len()
}'
{
  grep -m 1 '^#!\[warn(clippy::disallowed_types' crates/style-ir/src/lib.rs
  printf '%s\n' "$uses"
} > "$lints/core/src/lib.rs"
printf '%s\n' "$uses" > "$lints/host/src/lib.rs"
(cd "$lints" && cargo generate-lockfile --offline -q)
if output="$(cd "$lints" && cargo clippy -p core --offline -- -D warnings 2>&1)"; then
  echo "check-rust: clippy accepted hash maps, clocks, and randomness in a core crate" >&2
  exit 1
fi
for banned in type:std::collections::HashMap type:std::collections::HashSet \
  type:std::hash::RandomState method:std::hash::RandomState::new \
  type:std::time::SystemTime method:std::time::SystemTime::now \
  type:std::time::Instant method:std::time::Instant::now; do
  if ! grep -qF "use of a disallowed ${banned%%:*} \`${banned#*:}\`" <<< "$output"; then
    printf 'check-rust: clippy did not name %s in a core crate:\n%s\n' "${banned#*:}" "$output" >&2
    exit 1
  fi
done
(cd "$lints" && cargo clippy -p host --offline -q -- -D warnings)
echo "banned in the core, allowed in a host, as it should be"

step "Fuzz targets: formatting and clippy"
# The fuzz workspace is not a member of the main one; built here, an API change of the crates it
# fuzzes breaks the push, not next week's fuzz run.
cargo fmt --manifest-path fuzz/Cargo.toml --check
cargo clippy --manifest-path fuzz/Cargo.toml --locked --all-targets -- -D warnings

step "Build for wasm32-unknown-unknown"
browser=()
while IFS= read -r manifest; do
  name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -n 1)"
  [[ " ${NATIVE_ONLY[*]} " == *" $name "* ]] || browser+=(-p "$name")
done < <(find crates -mindepth 2 -maxdepth 2 -name Cargo.toml | sort)
cargo build --locked --target wasm32-unknown-unknown "${browser[@]}"

if [[ -d crates/bindings-wasm ]]; then
  step "WebAssembly bindings and their size"
  bash scripts/build-web.sh
  # Verifies: Q-5
  size="$(gzip -9 -c target/web/forager_bindings_wasm_bg.wasm | wc -c)"
  echo "gzipped .wasm: $size bytes (budget $WASM_BUDGET_BYTES)"
  if ((size > WASM_BUDGET_BYTES)); then
    echo "check-rust: the WebAssembly bundle exceeds its budget (Q-5)" >&2
    exit 1
  fi
fi

step "cargo deny"
cargo deny --locked check

step "cargo deny refuses a non-permissive licence"
# Verifies: ADR-0012, Q-6
# A fixture workspace whose dependency declares GPL-3.0-only; the licence check must fail on it.
fixture="$(mktemp -d)"
trap 'rm -rf "$lints" "$fixture"' EXIT
mkdir -p "$fixture/app/src" "$fixture/gpl/src"
cp deny.toml rust-toolchain.toml "$fixture/"
printf '[workspace]\nmembers = ["app"]\nresolver = "3"\n' > "$fixture/Cargo.toml"
printf '[package]\nname = "app"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n[dependencies]\ngpl = { path = "../gpl" }\n' \
  > "$fixture/app/Cargo.toml"
printf '[package]\nname = "gpl"\nversion = "0.1.0"\nedition = "2024"\nlicense = "GPL-3.0-only"\n' \
  > "$fixture/gpl/Cargo.toml"
touch "$fixture/app/src/lib.rs" "$fixture/gpl/src/lib.rs"
(cd "$fixture" && cargo generate-lockfile --offline -q)
# The check must fail, and fail on the licence: any other failure would pass for a refusal.
if output="$(cd "$fixture" && cargo deny check licenses 2>&1)"; then
  echo "check-rust: cargo deny accepted a GPL-3.0-only dependency" >&2
  exit 1
fi
if ! grep -q 'GPL-3.0-only' <<< "$output" || ! grep -q 'rejected' <<< "$output"; then
  printf 'check-rust: cargo deny failed for another reason than the licence:\n%s\n' "$output" >&2
  exit 1
fi
echo "refused, as it should be"
