# ADR-0043: The conformance test runs the engine's wasm32 build in wasmi and compares it with the native render

- **Status:** 🟡 proposed
- **Date:** 2026-11-07
- **Deciders:** Markus Brigl
- **Applies to:** the test of G-9 in `crates/cli/tests/`, its `wasmi` dev-dependency, the C-ABI exports of `crates/bindings-wasm` behind its feature `conformance`, the build of that module in `scripts/check-rust.sh`, and `deny.toml` for the crates it adds

## Context

G-9 asks that the same input rendered natively and on `wasm32-unknown-unknown` yields
bit-identical RGBA buffers; it backs Q-2, and it is the one goal no test cites. The browser
bundle is built for that target with wasm-bindgen (`crates/bindings-wasm`, `scripts/build-web.sh`),
but nothing runs it outside a browser, and [ADR-0008](0008-rust-toolchain.md) puts a browser test
runner out of scope. [ADR-0010](0010-rust-dev-container.md) installs no Node in the Dev Container,
and a test is to run there as on CI.

The bindings take mapfile text and GeoJSON per layer, and bind no shapefile
([ADR-0038](0038-browser-bindings-bind-geojson-only.md)); most cases in scope read shapefiles. The
comparison G-9 asks for is of the engine and its renderer; the mapfile frontend and the readers of
data files are held by their own tests.

The candidates, read on 2026-11-07: wasmi 2.0.0 (an interpreter, MIT OR Apache-2.0, MSRV 1.86, a
small tree, two published security audits), wasmtime 50.0.1 (Cranelift, Apache-2.0 WITH
LLVM-exception, which `deny.toml` does not admit, a heavy build, a new major version about every
month), wasmer 7.5.0 (several compiler backends, the largest tree), and Node with wasm-bindgen's
glue (no Node in the container; the browser runner ADR-0008 left out). None of the Rust runtimes
runs wasm-bindgen's glue without stubbing its imports, which follow wasm-bindgen's internals.

## Decision

We will run the engine's `wasm32-unknown-unknown` build in `wasmi`, a dev-dependency of the
conformance test, through plain C-ABI exports (`alloc`, `dealloc`, `render`) that
`crates/bindings-wasm` compiles behind its feature `conformance` with the browser bundle's `wasm`
profile, handing them each case as a style document with its features inline
([ADR-0033](0033-inline-features-in-the-style-ir.md)) and the fonts its labels name, and compare
the RGBA buffer byte for byte with the native render of the same document.

**Out of scope:** the browser bundle and its JavaScript API, which the feature leaves unchanged
(Q-5); the mapfile frontend and the readers of data files on wasm32, which the inline document
keeps out of the comparison; what to do if the platform's mathematical functions and `libm` part
in the last bit on the two targets, a decision of its own once the test shows it; a browser test,
which G-14 waits for.

## Alternatives considered

- **wasmtime** — fast, and the reference implementation, but its licence needs an exception in
  `deny.toml` ([ADR-0012](0012-crate-supply-chain.md)), its build costs minutes cold, and its
  monthly major versions bring advisory churn, for speed this test does not need.
- **wasmer** — the heaviest tree, and nothing this test needs beyond wasmi.
- **Node running the wasm-bindgen bundle** — tests the glue as shipped, but supersedes ADR-0010
  and takes up what ADR-0008 left out, and the module has no file system, so every case would be
  embedded and the native buffers passed between processes.
- **Stubbing wasm-bindgen's imports in a Rust runtime** — no new exports, but bound to
  wasm-bindgen's internals, which change with its versions.
- **A host crate of its own for the exports** — keeps `bindings-wasm` as it is, but supersedes
  [ADR-0009](0009-ports-and-adapters-crates.md) to add a crate for the same logic.
- **Mapfile and GeoJSON as the input** — covers the frontend on wasm32 too, but the shapefile cases
  cannot be bound in the bindings (ADR-0038), and a difference would point at the frontend, the
  readers, or the engine alike.

## Sources / Prior art

- crates.io, checked 2026-11-07: <https://crates.io/crates/wasmi>,
  <https://crates.io/crates/wasmtime>, <https://crates.io/crates/wasmer>,
  <https://crates.io/crates/wasm-bindgen-test>, with their dependency lists.
- wasmi README (supported proposals, audits): <https://github.com/wasmi-labs/wasmi>; benchmarks:
  <https://github.com/wasmi-labs/wasmi-benchmarks>.
- wasmtime `Config` (NaN canonicalization, relaxed SIMD):
  <https://docs.rs/wasmtime/50.0.1/wasmtime/struct.Config.html>.
- wasm-bindgen-test usage:
  <https://wasm-bindgen.github.io/wasm-bindgen/wasm-bindgen-test/usage.html>.
- rustc, `wasm32-unknown-unknown`:
  <https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html>.
- The ticket `docs/plan/conformance-harness/10-native-and-wasm-alike.md` and the maintainer's
  answers of 2026-11-07.

## Consequences

- Positive: G-9 gets its test, run in the Dev Container as on CI, with no second toolchain and no
  licence exception; a difference points at the engine or the renderer.
- Positive: no new crate, so ADR-0009 stands; the browser bundle does not change.
- Negative / trade-offs: an interpreter runs the renders several times slower than native; the
  test may have to run in a scheduled workflow rather than on every push, which its first run
  decides.
- Negative / trade-offs: the C-ABI exports are a second, test-only face of `bindings-wasm`, built
  only with its feature.
- Follow-ups: whether `render-agg` and `render` take their mathematical functions from `libm` on
  both targets, as `symbolize` does, should the test find a difference there.

## Enforcement

The test that renders every case in scope through the module and compares the buffers cites G-9
and this ADR with `Verifies: G-9, ADR-0043`. `crates/arch-test` refuses `wasmi` outside the
dev-dependencies of `crates/cli`, and `cargo deny` holds its licence.
