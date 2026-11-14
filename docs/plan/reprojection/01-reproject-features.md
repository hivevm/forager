# Ticket: A layer in another projection is drawn in the map's (tracer bullet)

## What to build

A layer whose `PROJECTION` differs from the map's is drawn in the map's projection: each feature's
shape, inline features included, is reprojected after the source delivers it and before the
layer's `GEOMTRANSFORM`, classes, and labels. The source is asked for the map's extent projected
into the layer's projection; this ticket may project the extent's corners and edges simply, and
ticket 03 makes it MapServer's. Before any code, `propose-adr` runs for the reprojection in the
core crate `proj`; the session stops once the ADR is proposed and resumes after the human's
review. The ADR also settles where the msautotest harness takes real definitions for the
EPSG codes of the cases it brings into scope: today it resolves them to stand-ins in longitude and
latitude, and ADR-0019 rejected a subset of the EPSG dataset in the repository for its licence.

## Acceptance criteria

- [ ] Given a proposed ADR for the reprojection in `proj`, reviewed by the human, when the work
  starts, then it settles: the dependency under ADR-0011, its build for `wasm32-unknown-unknown`
  without C code (Q-1), its licence (Q-6), the bundle's growth against Q-5, and how its
  trigonometry gives the same bits on every target where std's calls the platform's libm (Q-2),
  and where the harness's real EPSG definitions come from.
- [ ] Given a map in `init=epsg:4326` and a layer in `init=epsg:3978`, as msautotest's
  `misc/quoting.map`, when the case is rendered, then it is in scope and holds its outcome by the
  metric of G-6, natively and on wasm32 (G-9).
- [ ] Given a layer without a projection, or in the map's, when it is rendered, then its features
  are not reprojected, and every case in scope holds its outcome.
- [ ] Given a layer in another projection, when the engine asks its source, then the area asked
  covers the map's extent in the source's coordinates.
- [ ] Given a point that does not reproject, when the layer is drawn, then the point is left out
  with a diagnostic naming the layer, and no panic occurs (Q-3).

## Read first

- Touches: `crates/proj` (new, the row ADR-0008 names), `crates/engine/src/lib.rs`,
  `crates/engine/src/classes.rs`, `crates/engine/src/inline.rs`, `Cargo.toml`, `deny.toml`,
  `crates/cli/tests/msautotest.rs`, `crates/cli/tests/conformance/cases.txt`, `clippy.toml`,
  `crates/{engine,label,symbolize}/clippy.toml`, `crates/arch-test/tests/dependencies.rs`,
  `crates/render/src/hatch.rs`, `crates/render-agg/src/lib.rs`, `scripts/check-notice.sh` and its
  test, `THIRD-PARTY-NOTICES.md`, `docs/ARCHITECTURE.md`, `docs/semantics/projection.md` (the
  reprojection it now describes)
- `docs/semantics/projection.md`; MapServer's `msDrawLayer`, `msDrawShape`, `msProjectShapeEx`,
  and `msProjectionsDiffer` (`src/mapdraw.c`, `src/mapproject.c`) at the pinned tag
- Glossary: Core, Feature source, Viewport; `docs/CONVENTIONS.md`
- Seam: the msautotest harness (`crates/cli/tests/msautotest.rs`) and the engine's render with
  sources in memory

## Blocked by

- None (can start immediately)

## Mode

with the human: it starts with an ADR the human reviews, and it adds a dependency to the core.
