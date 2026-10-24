#!/usr/bin/env bash
# Builds, tests, and lints the Rust workspace (ADR-0008): formatting, clippy with warnings as
# errors, the tests, the load of the msautotest suite at the pinned MapServer release (G-1,
# fetched once into target/ by scripts/fetch-msautotest.sh), the build of every crate that must
# run in the browser for wasm32-unknown-unknown (Q-1), the WebAssembly bindings and their size
# (Q-5) once they exist, and cargo deny (ADR-0012). scripts/check-all.sh and the `rust` job of
# checks.yml run it; the tools come from scripts/install-tools.sh.
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

step "msautotest: every mapfile loads without an error (G-1)"
suite="$(bash scripts/fetch-msautotest.sh)"
FORAGER_MSAUTOTEST="$suite" cargo test -p forager-cli --locked --test msautotest -- --ignored

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
trap 'rm -rf "$fixture"' EXIT
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
