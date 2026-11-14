# Ticket: Shapes are cut at the horizon of an orthographic map (RFC 5)

## What to build

A line reprojected into a projection where part of it lies beyond the horizon, such as an
orthographic one, is split into the parts that reproject, each ended at the horizon as
`msProjectShapeLine` finds it; a polygon is closed along the horizon as it closes it. The points the
horizon search adds count against a bound per layer (Q-9).

## Acceptance criteria

- [ ] Given msautotest's `misc/ortho.map` and `misc/bug1526.map`, when they are rendered, then they
  are in scope and hold their outcome by the metric of G-6, natively and on wasm32 (G-9).
- [ ] Given a line that leaves and re-enters the visible hemisphere, when it is reprojected, then
  it becomes the parts and horizon points `msProjectShapeLine` gives at the pinned tag.
- [ ] Given a layer whose horizon search would add more points than its bound, when it is drawn,
  then the layer stops at the bound with a diagnostic, and a test feeds such an input (Q-9).

## Read first

- Touches: `crates/proj`, `crates/engine/src/lib.rs`, `crates/cli/tests/conformance/cases.txt`,
  `docs/SPECIFICATION.md` (Q-9, if the bound is new)
- MapServer's `msProjectShapeLine` and its horizon search (`src/mapproject.c`) at the pinned tag
- Seam: the msautotest harness; a test of `crates/proj`

## Blocked by

- docs/plan/reprojection/03-search-rect-as-mapserver.md

## Mode

with the human: a new bound of Q-9 is a change to the specification, which the human lands.
