# ADR-draft-reprojection-ported-in-proj: The core crate `proj` reprojects with its own port of PROJ's formulas, every transcendental function taken from `libm`, and no crate beyond it

- **Status:** 🟡 proposed
- **Date:** 2026-11-14
- **Deciders:** Markus Brigl
- **Applies to:** `crates/proj`, its `[dependencies]`, the reprojection of a layer's features and of the area asked of its source in `crates/engine`, the workspace's `clippy.toml` and the three crate copies of it (`crates/engine/clippy.toml`, `crates/label/clippy.toml`, `crates/symbolize/clippy.toml`), `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of PROJ, and the EPSG table the msautotest harness `crates/cli/tests/msautotest.rs` passes to the CLI

## Context

A layer whose `PROJECTION` differs from the map's is drawn by MapServer in the map's projection
(`msDrawShape` in `src/mapdraw.c` calls `msProjectShapeEx` in `src/mapproject.c`). Forager reads
every `PROJECTION` into the style IR as a proj4 string
([ADR-0019](0019-projection-resolver.md)) and reprojects nothing
(`docs/plan/reprojection/spec.md`). [ADR-0008](0008-ports-and-adapters-crates.md) already names
the core crate `proj`, which may depend on `style-ir` alone. The plan assumed proj4rs inside it,
as the specification does twice: G-11 says "proj4rs shall agree with PROJ", and its assumptions
list proj4rs among the crates the work leans on.

**proj4rs, read at 0.3.0** (2026-10-26, `MIT OR Apache-2.0`, 3liz). It is pure Rust and implements
`lcc`, `merc`, `tmerc`, `ortho`, and `+over`, among 29 projections. Its normal dependencies are
`thiserror` and `proj4rs-geodesic`. For `wasm32-unknown-unknown` it is bound to JavaScript by
`cfg(target_arch = "wasm32", target_os = "unknown")`, with no feature to turn that off:

- `src/parse.rs` parses every number of a proj4 string with JavaScript's `parseFloat` from
  `js-sys`, to save 20 kB of `.wasm`;
- `src/wasm/` exports its own wasm-bindgen API (`Projection` and the grid loaders);
- its manifest takes `js-sys`, `web-sys`, `wasm-bindgen`, and `console_log` on that target,
  and `proj4rs-geodesic` takes `wasm-bindgen`.

So, unchanged, it breaks three decisions in force:

- [ADR-0008](0008-ports-and-adapters-crates.md): `js-sys` and `web-sys` enter the core's tree,
  which only hosts and adapters may use; the test of `crates/arch-test` reads `cargo metadata`
  for every target and fails.
- [ADR-0034](0034-wasm-renders-compared-in-wasmi.md): every import of the conformance module traps,
  so `parseFloat` aborts the wasm32 render of every reprojected case.
- Q-5 and the browser bindings: its exports join the bundle and its JavaScript API.

**Determinism (Q-2, G-9).** proj4rs calls std's `f64` methods about 600 times: `sin`, `cos`,
`sin_cos`, `tan`, `atan2`, `asin`, `atan`, `powf`, `hypot`, `ln`, `exp`, and others. Natively std
calls the platform's libm (glibc on Linux). `wasm32-unknown-unknown` has no platform libm, so the
toolchain brings an implementation of its own. Nothing promises the two alike in the last bit. The
workspace already
answers this for its own code: it takes `sin`, `cos`, and `acos` from the `libm` crate without
its `arch` feature (the comment above `libm` in `Cargo.toml`), and the workspace's `clippy.toml`
bans `f64::hypot`. A reprojected coordinate goes through a dozen such calls before it is a pixel,
and one differing bit can move an edge across a cell of the rasterizer.

**What the cases need.** The msautotest cases the plan brings into scope use `lcc` (EPSG:3978,
`misc/quoting.map`), longitude and latitude, `merc` and `tmerc` through EPSG:3857 and the UTM
zones `adapter-epsg` builds in, and `ortho` with `+over` for the late tickets. Datum grids are out
of scope ([ADR-0019](0019-projection-resolver.md) refuses a definition that names one).

**Where the harness's definitions come from.** `crates/cli/tests/msautotest.rs` passes the CLI an
EPSG table (`epsg_table`) that maps eleven codes to stand-ins in longitude and latitude, enough to
load and not to draw. [ADR-0019](0019-projection-resolver.md) rejected a subset of the EPSG
dataset in the repository for IOGP's terms. The suite MapServer ships at the pin holds its own
init file, `msautotest/wxs/data/epsg2`. It defines EPSG:3978 as `+proj=lcc +lat_1=49 +lat_2=77
+lat_0=49 +lon_0=-95 +x_0=0 +y_0=0 +ellps=GRS80 +datum=NAD83 +units=m +no_defs`.
It holds none of the ten other codes. The harness already reads that file for the mapfiles that
name it by a path.

The maintainer chose on 2026-11-14 a port of its own over proj4rs, and the suite's `epsg2` as the
harness's source.

## Decision

We will reproject in `proj` by our own port of PROJ's formulas, with `libm`'s transcendental
functions and no other dependency but `style-ir`.

- **Input.** `proj` must take a projection as the proj4 string of the style IR. It parses the
  string itself.
- **Scope.** `proj` implements a projection, a parameter, or a datum only when a case in scope or a
  ticket needs it. The first are longitude and latitude, `lcc`, `merc`, `tmerc` with `+zone`, and
  the datum shift by `+towgs84` of three or seven parameters.
- **Unsupported.** A projection or a parameter `proj` does not implement must be an error value
  naming it. The engine reports it as a diagnostic naming the layer, and does not draw that layer.
- **Failure.** A point that does not transform must be an error value, never a panic (Q-3).
- **Mathematics.** `proj` must take `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `exp`,
  `log`, `pow`, `sinh`, `cosh`, `tanh`, `atanh`, `cbrt`, and `hypot` from `libm`. It may use
  `f64::sqrt` and IEEE's arithmetic, which every target rounds alike.
- **Reference.** The port follows PROJ's sources at the version of the pinned MapServer build.
  Each file that ports a PROJ source must name it and hold PROJ's MIT notice.
- **Harness.** The msautotest harness must pass the suite's `wxs/data/epsg2` as the CLI's EPSG
  table. It keeps a stand-in only for a code that file lacks.

**Out of scope:**

- the area asked of a source as `msProjectRect` computes it, horizon clipping into `ortho`, and
  the dateline of `+over`, each a ticket of the plan;
- the tolerance of G-11, which a measurement against PROJ sets;
- datum grids, WKT, PROJJSON, PROJ pipelines, and raster reprojection.

## Alternatives considered

- **proj4rs from crates.io, as it is.** It would break [ADR-0008](0008-ports-and-adapters-crates.md)
  and [ADR-0034](0034-wasm-renders-compared-in-wasmi.md), as the Context shows. Taking it would
  mean superseding the first for `js-sys` in the core and giving the conformance module a
  `parseFloat` import. Its std calls would still leave G-9 to luck.
- **A pruned copy of proj4rs in the workspace.** About 13 000 lines without its wasm and grid
  modules, `js-sys` removed and std's functions replaced by `libm`. It brings 29 projections where
  the plan needs five, and a fork to keep in step by hand.
  [ADR-0035](0035-vendored-third-party-files.md) covers vendored files only for `examples/web/`.
- **A pull request to proj4rs** with features that turn off its wasm module and take `libm`. Ticket
  01 would wait for an upstream release of unknown date. The pull request stays open to anyone,
  and a later ADR may supersede this one for the crate once it lands.
- **PROJ itself, through `proj-sys` or a WebAssembly build of it.** It links C++ code, which
  [ADR-0008](0008-ports-and-adapters-crates.md) and Q-1 refuse in the core.
- **proj4rs with std's functions, and the G-9 test as the judge.**
  [ADR-0034](0034-wasm-renders-compared-in-wasmi.md) left this open until the test shows a
  difference. A case that passes today can fail on a new toolchain or glibc, with nothing to fix
  but the dependency.
- **The harness's definitions generated from PROJ at the pinned build**, into `target/` and not
  committed. Every run of `scripts/check-all.sh`, and the `rust` job of CI, would then need PROJ
  installed, as only `noise.yml` has it today.
- **The definitions written into the test with IOGP's acknowledgement.** The terms allow
  extracts. It is the subset in the repository that [ADR-0019](0019-projection-resolver.md)
  rejected.

## Sources / Prior art

- proj4rs 0.3.0, its source as published on crates.io, read 2026-11-14: `Cargo.toml`,
  `src/parse.rs`, `src/wasm/mod.rs`, `src/lib.rs`, `src/projections/`.
  <https://crates.io/crates/proj4rs>, <https://github.com/3liz/proj4rs>.
- proj4rs-geodesic 0.2.0, its manifest, read 2026-11-14.
- PROJ, <https://proj.org>, MIT licence; its `src/projections/lcc.cpp`, `merc.cpp`, `tmerc.cpp`,
  and `src/transformations/helmert.cpp` are the reference the port follows.
- MapServer at the pin, `src/mapproject.c` (`msProjectShapeEx`, `msProjectRect`,
  `msProjectionsDiffer`, `msProjectCreateReprojector`) and `src/mapdraw.c` (`msDrawLayer`,
  `msDrawShape`).
- The msautotest suite at the pin, `msautotest/wxs/data/epsg2` and `misc/quoting.map`.
- The `libm` crate's comment in the workspace's `Cargo.toml`, and
  [ADR-0026](0026-agg-blender-and-resampler.md) on the same question for the rasterizer.

## Consequences

- Positive: Q-1 and Q-6 hold without a new crate. `proj` adds only the formulas the cases use to
  the bundle (Q-5), measured when the first lands. G-9 holds by construction, not by the test's
  luck. Errors are this crate's own values, so a malformed proj4 string or a point past a
  projection's domain is a diagnostic (Q-3).
- Negative / trade-offs: every projection the cases bring is code to port and test. The port can
  diverge from PROJ where proj4rs would not, so G-11's measurement against PROJ is the guard, not
  an option. A user's mapfile in a projection `proj` lacks draws nothing of that layer until a
  ticket adds it. A case whose codes `epsg2` lacks stays out of scope.
- Follow-ups:
  - **The specification.** G-11 and the assumption on the crates the work leans on name proj4rs. A
    human lands the change that names `proj` instead.
  - **The plan.** The reprojection spec and tickets 01 and 02 name proj4rs and change with it.
  - **The other crates on the output path.** The bans of the mathematics in `clippy.toml` hold
    them too; two tests, in `crates/render/src/hatch.rs` and `crates/render-agg/src/lib.rs`, call
    std's `sin` and `cos` today and move to `libm`.
  - **The notice of PROJ** joins `THIRD-PARTY-NOTICES.md`, and `scripts/check-notice.sh` learns the
    files that name a PROJ source.

## Enforcement

- The test of `crates/arch-test` holds the edges of [ADR-0008](0008-ports-and-adapters-crates.md).
  A test there cites this ADR with `Verifies:`. It fails when `proj` declares a normal dependency
  other than `forager-style-ir` and `libm`.
- The workspace's `clippy.toml` bans the `f64` and `f32` methods the *Mathematics* entry lists,
  each with the reason Q-2. The three crate copies carry the same bans, which arch-test holds equal.
- The msautotest harness renders `misc/quoting.map` with the suite's `epsg2`, natively and through
  the wasm32 module, and compares the buffers (G-9). That test cites this ADR with `Verifies:`.
- Tests of `crates/proj` cite this ADR with `Verifies:`. They expect the error value for an
  unknown projection, an unknown parameter, and a point that does not transform.
- `scripts/check-notice.sh` fails on a file under `crates/proj` that names a PROJ source without
  PROJ's notice.
