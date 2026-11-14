# ADR-0034: The conformance test runs the engine's wasm32 build in wasmi and compares it with the native render

- **Status:** 🟢 accepted
- **Date:** 2026-11-07
- **Deciders:** Markus Brigl
- **Applies to:** the test of G-9 in `crates/cli/tests/`, its `wasmi` dev-dependency and wasmi's optimization in the dev profile, the exports of `crates/bindings-wasm` behind its feature `conformance`, the build of that module in `scripts/check-rust.sh`, and `cargo deny` for the crates it adds

## Context

G-9 asks that the same input rendered natively and on `wasm32-unknown-unknown` yields
bit-identical RGBA buffers; it backs Q-2, and the specification's *Strategy* ends each phase with
it green. The browser bundle is built for that target with wasm-bindgen (`crates/bindings-wasm`,
`scripts/build-web.sh`), but nothing runs it outside a browser, and no test runs a browser: a
browser test waits for G-14, as *Out of scope* below says.
[ADR-0009](0009-dev-container.md) installs no Node in the dev container, and a test is to run there
as on CI.

The bindings take mapfile text and GeoJSON per layer, and bind no shapefile
([ADR-0024](0024-browser-bindings-bind-geojson-only.md)); most cases in scope read shapefiles. The
comparison G-9 asks for is of the engine and its renderer; the mapfile frontend and the readers of
data files are held by their own tests.

The candidates, read on 2026-11-07: wasmi 2.0.0 (an interpreter, MIT OR Apache-2.0, MSRV 1.86, a
small tree, two published security audits), wasmtime 50.0.1 (Cranelift, Apache-2.0 WITH
LLVM-exception, which `deny.toml` does not admit, a heavy build, a new major version about every
month), wasmer 7.5.0 (several compiler backends, the largest tree), and Node with wasm-bindgen's
glue (no Node in the container, [ADR-0009](0009-dev-container.md); no browser test before G-14).
None of the Rust runtimes runs wasm-bindgen's glue without stubbing its imports, which follow
wasm-bindgen's internals.

The workspace denies `unsafe` code outside `render-agg` ([ADR-0025](0025-agg-rust-rasterizer.md)),
and an export of the C ABI needs `#[unsafe(no_mangle)]`; `wasm_bindgen` exports a function without
it in the crate's code. Its exports in the module `cargo` builds carry a hash in their names, which
the wasm-bindgen CLI removes, as it does for the bundle (read 2026-11-10, wasm-bindgen 0.2.129).

## Decision

We will run the engine's `wasm32-unknown-unknown` build in `wasmi` and compare each case's RGBA
buffer byte for byte with the native render of the same document.

- **The runtime.** `wasmi` must be a dev-dependency of the conformance test alone.
- **The exports.** `crates/bindings-wasm` compiles exports of plain integers
  (`conformance_input`, `conformance_render`, `conformance_output`) with `wasm_bindgen`, behind
  its feature `conformance`, with the browser bundle's `wasm` profile.
- **The module.** The module passes through the wasm-bindgen CLI as the bundle does. Each import
  of the module is a function that traps.
- **The input.** The test hands the module each case as a style document with its features inline
  ([ADR-0018](0018-inline-features.md)), and the files its native render read, its fonts and
  images.

**Out of scope:** the browser bundle and its JavaScript API, which the feature leaves unchanged
(Q-5); the mapfile frontend and the readers of data files on wasm32, which the inline document
keeps out of the comparison; what to do if the platform's mathematical functions and `libm` part
in the last bit on the two targets, a decision of its own once the test shows it; a browser test,
which G-14 waits for.

## Alternatives considered

- **wasmtime** — fast, and the reference implementation, but its licence needs an exception in
  `deny.toml` ([ADR-0011](0011-crate-supply-chain.md)), its build costs minutes cold, and its
  monthly major versions bring advisory churn, for speed this test does not need.
- **wasmer** — the heaviest tree, and nothing this test needs beyond wasmi.
- **Node running the wasm-bindgen bundle** — tests the glue as shipped, but supersedes
  [ADR-0009](0009-dev-container.md) and takes up what [ADR-0007](0007-rust-toolchain.md) left out,
  and the module has no file system, so every case would be embedded and the native buffers passed
  between processes.
- **Stubbing wasm-bindgen's imports in a Rust runtime** — no new exports, but bound to
  wasm-bindgen's internals, which change with its versions. The exports chosen call none of the
  imports, so a stub that traps stands for each, whatever wasm-bindgen imports.
- **C-ABI exports with `#[unsafe(no_mangle)]`** — no wasm-bindgen in the comparison, but `unsafe`
  code in a second crate, against [ADR-0025](0025-agg-rust-rasterizer.md).
- **The exports of the module `cargo` builds, found by their names' prefix** — no pass of the
  wasm-bindgen CLI, but bound to the hash it appends, an internal of wasm-bindgen.
- **A host crate of its own for the exports** — keeps `bindings-wasm` as it is, but supersedes
  [ADR-0008](0008-ports-and-adapters-crates.md) to add a crate for the same logic.
- **Mapfile and GeoJSON as the input** — covers the frontend on wasm32 too, but the shapefile cases
  cannot be bound in the bindings ([ADR-0024](0024-browser-bindings-bind-geojson-only.md)), and a
  difference would point at the frontend, the readers, or the engine alike.

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
- The conformance-harness spec's ticket of the native and wasm renders alike, deleted with its
  implementation, and the maintainer's answers of 2026-11-07.
- wasmi's features `auto-dispatch` and `portable-dispatch`, in its crate documentation at 2.0.0.

## Consequences

- Positive: G-9 has its test, run in the dev container as on CI, with no second toolchain and no
  licence exception; a difference points at the engine or the renderer.
- Positive: no new crate, so [ADR-0008](0008-ports-and-adapters-crates.md) stands; the browser
  bundle does not change.
- Negative / trade-offs: an interpreter runs the renders several times slower than native. The
  first run compared the 74 cases in scope in 14 s, with wasmi optimized in the dev profile and its
  portable dispatch, so the test runs on every push; unoptimized it took nine minutes, and the
  dispatch by tail calls overflowed the stack.
- Negative / trade-offs: the exports are a second, test-only face of `bindings-wasm`, built only
  with its feature; the comparison reaches the engine through `wasm_bindgen`'s export of integers,
  not through the bundle's JavaScript API.
- Negative / trade-offs: the native side's render of a document with its files, and the module's
  input format, are written twice, in `bindings-wasm` and in the CLI's test, since
  [ADR-0008](0008-ports-and-adapters-crates.md) lets no host depend on another. A drift of the
  format fails the comparison; a drift of the render, which is a call of
  `forager_engine::render_with_files` with no source bound and a fresh `AggRenderer` on both sides,
  a reviewer holds. The maintainer kept the copies (2026-11-10).
- Follow-ups: whether `render-agg` and `render` take their mathematical functions from `libm` on
  both targets, as `symbolize` does, should the test find a difference there; its first run found
  none (2026-11-10).

## Enforcement

The test of `crates/cli/tests/msautotest.rs` that renders every case in scope through the module
and compares the buffers cites G-9, Q-2, and this ADR with `Verifies:`; `scripts/check-rust.sh`
builds the module and runs it. Unit tests in
`crates/cli/tests/conformance/tests.rs` cite this ADR; one decides that a difference names the
first differing pixel. `crates/arch-test/tests/dependencies.rs` refuses `wasmi` outside the
dev-dependencies of `crates/cli`, and `cargo deny` holds its licence.
