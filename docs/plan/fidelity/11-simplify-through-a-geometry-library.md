# Ticket: `simplify` and `simplifypt` through a geometry library (ADR first)

## What to build

A `GEOMTRANSFORM` of `simplify` or `simplifypt` is drawn as MapServer draws it through GEOS. Today
the transform tree holds both as `not_drawn`, and the engine warns. This ticket brings the geometry
code into the core, with `simplify` as its tracer bullet. Ticket 12 adds `buffer` on the same code,
and ticket 10 needs both functions on a style.

MapServer at the pin computes them in `src/mapgeos.c`. `msGEOSSimplify` (`:922`) calls
`GEOSSimplify_r`, GEOS's Douglas-Peucker simplifier. `msGEOSTopologyPreservingSimplify` (`:946`)
calls `GEOSTopologyPreserveSimplify_r`. `msGEOSBuffer` (`:899`) calls `GEOSBuffer_r` with 30
segments a quadrant. `src/mapparser.y` (`:842`, `:856`, `:758`) binds them as shape functions. The
noise run builds MapServer against the runner's `libgeos-dev` (`scripts/build-mapserver.sh`). That
GEOS is the reference the cases are held to.

**`propose-adr` runs first.** The ADR decides which code computes `simplify`, `simplifypt`, and
`buffer` in the core. It weighs these:

- **The `geo` crate** (georust): Douglas-Peucker `Simplify`, and Visvalingam-Whyatt
  `SimplifyVwPreserve` for topology. GEOS's topology-preserving simplifier is a Douglas-Peucker
  variant, so the VW variant may not reproduce MapServer's points. Its buffer support, its
  dependency tree (it may already share `i_overlay` with ADR-0028), its build for wasm32 without C
  (Q-1), and its gzipped size in the bundle (Q-5) are to be verified, not recalled.
- **A port of GEOS's algorithms** (`DouglasPeuckerSimplifier`, `TopologyPreservingSimplifier`, and
  `BufferOp` with `OffsetCurveBuilder`): closest to MapServer, sized by what the cases need, under
  the LGPL-2.1 of GEOS. That licence conflicts with Q-6 and the *Constraints* for a port. The ADR
  says whether a clean-room port from the published algorithm is possible, or whether this option is
  rejected.
- **Another crate**, such as one built on `i_overlay`'s outline offsetting, if one fits better. The
  licence is checked with `cargo deny` (ADR-0011, Q-6).

The ADR states how close each option comes to GEOS on the cases' shapes. It also states where the
code lives under ADR-0008: in `symbolize`, beside the layer's transform, or in a crate of its own.

**ADR-0036 needs a successor.** Its *Decision* lists the tree's nodes and gives `simplify`,
`simplifypt`, and `buffer` as `not_drawn` reasons. Adding them as nodes changes an accepted
decision, so a proposed successor that supersedes ADR-0036 travels with the library ADR. It covers
the nodes, their arguments, and the schema. No release tag holds a schema yet, so ADR-0014 lets the
change regenerate the current version's file. A version move is the maintainer's choice.

Case: `renderers/geomtransform-vertex.map geomtransform-vertex-layer.png` (`-l vertex2`): a line
layer of `GEOMTRANSFORM (simplify([shape],25))`.

## Acceptance criteria

- [ ] Given the ADR of the geometry code and the successor of ADR-0036, when they are proposed, then
  they stand in commits of ADRs only, handed to the maintainer before anything is built against
  them.
- [ ] Given `geomtransform-vertex-layer.png`, when the harness renders it, then it passes by the
  metric of G-6, natively and bit-identically on wasm32 (G-9), and the list says `geometry passes`.
- [ ] Given a mapfile with `simplify([shape], n)` or `simplifypt([shape], n)` in a `GEOMTRANSFORM`,
  when the frontend loads it, then the tree holds the node, and the style document round-trips
  (G-3).
- [ ] Given a line and a polygon, when `simplify` and `simplifypt` compute them, then each yields
  the points GEOS yields at the pin's build. Unit tests hold coordinates taken from GEOS.
- [ ] Given the core with the geometry code, when it is built for `wasm32-unknown-unknown`, then it
  links no C, `cargo deny check` passes, and the bundle stays under Q-5's 3 MB. The hand-off states
  the growth of the bundle.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `docs/adr/` (two drafts), `crates/style-ir/src/transform.rs`,
  `schema/style-document-1.1.json`, `crates/mapfile/src/translate/` (the translation of
  `GEOMTRANSFORM` expressions), `crates/symbolize/src/layer_transform.rs`,
  `crates/symbolize/Cargo.toml` or a new crate, `Cargo.toml`, `Cargo.lock`,
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapgeos.c` (`msGEOSSimplify`, `msGEOSTopologyPreservingSimplify`,
  `msGEOSBuffer`), `src/mapparser.y` (the shape functions)
- Glossary: Transform tree
- `docs/adr/0036-typed-geomtransform.md` (whole, since it is superseded),
  `docs/adr/0008-ports-and-adapters-crates.md`, `docs/adr/0011-crate-supply-chain.md`,
  `docs/adr/0014-style-document-versions.md`, `docs/adr/0028-hatch-cut-by-i-overlay.md`
- `docs/SPECIFICATION.md` (Q-1, Q-5, Q-6, Q-9's bounds on the points a `GEOMTRANSFORM` adds and
  visits)
- Skill: `propose-adr`
- Golden path: `generalize` in `crates/symbolize/src/layer_transform.rs`, from the frontend's
  translation to the layer's draw
- Seam: the msautotest harness; the symbolizer's unit tests; the frontend's `load` with files in
  memory; `crates/style-ir/tests/schema.rs`

## Blocked by

- None (can start immediately)

## Mode

with the human: it starts with two ADRs, adds a dependency, and changes the style document, which
ADR-0006 keeps with the maintainer. The session stops after the ADRs' commits and the hand-off.
After the review, it builds `simplify` first. If `simplifypt` does not fit the rest of the session,
the session hands off with it as a ticket of its own, blocking ticket 10.
