# Ticket: `GEOMTRANSFORM` buffer

## What to build

A style of `GEOMTRANSFORM (buffer([shape], n))` draws the buffered shape as MapServer's GEOS draws
it, for a positive and a negative distance. `msGEOSBuffer` (`src/mapgeos.c:899`) calls
`GEOSBuffer_r` with 30 segments a quadrant, and `src/mapparser.y:758` binds it. The geometry code is
the one ticket 11's ADR chose. `buffer` becomes a node of the tree that ADR-0036's successor adds,
next to `simplify`.

Case: `renderers/polygon-as-line-offset.map polygon-as-line-offset.png`. Its polygons are outlined
by styles of `buffer([shape], 1)`, `buffer([shape], -1)`, and `OFFSET 10 10` and `-10 -10`. These
are styles of a class, so the case also needs a style's drawn expression from ticket 10. The buffer
works on the shape in pixels, as `msDrawTransformedShape` hands it on.

A buffer adds points. Q-9 bounds the points one layer's `GEOMTRANSFORM` adds (1,000,000) and the
points it visits beyond reading each once (100,000,000). The buffer is held to both, as the layer's
transforms are.

## Acceptance criteria

- [ ] Given `renderers/polygon-as-line-offset.map`, when the harness renders it, then it passes by
  the metric of G-6, natively and bit-identically on wasm32 (G-9), and the list says
  `geometry passes`.
- [ ] Given a mapfile with `buffer([shape], n)` in a `GEOMTRANSFORM`, when the frontend loads it,
  then the tree holds the node, and the style document round-trips (G-3).
- [ ] Given a polygon, when it is buffered by 1 and by -1, then the result's rings are those GEOS
  yields at the pin's build with 30 segments a quadrant, within the tolerance the ADR of ticket 11
  states. Unit tests hold coordinates taken from GEOS.
- [ ] Given a buffer that would add more points than Q-9 allows, when it is drawn, then the engine
  reports the bound's diagnostic and the style draws nothing.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/style-ir/src/transform.rs`, `schema/style-document-1.1.json`,
  `crates/mapfile/src/translate/` (the translation of `GEOMTRANSFORM` expressions), the geometry
  code ticket 11 placed, `crates/symbolize/src/layer_transform.rs`,
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapgeos.c` (`msGEOSBuffer`), `src/mapparser.y`,
  `src/mapgeomtransform.c` (`msDrawTransformedShape`)
- The ADRs ticket 11 proposed, and `docs/adr/0014-style-document-versions.md`
- `docs/SPECIFICATION.md` (Q-9, *Drawing*)
- Golden path: `simplify` as ticket 11 built it
- Seam: the msautotest harness; the unit tests of the geometry code; the frontend's `load` with
  files in memory; `crates/style-ir/tests/schema.rs`

## Blocked by

- docs/plan/fidelity/11-simplify-through-a-geometry-library.md
- docs/plan/fidelity/10-style-geomtransform-expressions.md

## Mode

with the human: it changes the style document and holds the new code to Q-9's resource limits.
ADR-0006 keeps both with the maintainer. The session stops before it commits the tree's new node and
the schema, for the maintainer's review. It also stops when the buffer's rings differ from GEOS's
beyond the ADR's tolerance after two attempts, and hands off with the difference measured.
