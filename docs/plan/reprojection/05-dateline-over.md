# Ticket: Shapes crossing the dateline keep their shape, `+over` included

## What to build

A line or polygon reprojected into longitude and latitude across the dateline is wrapped as
`msTestNeedWrap` and the dateline handling of MapServer's reprojection wrap it, and a projection
with `+over` keeps longitudes beyond ±180 as MapServer keeps them, in the shapes and in the area
asked of a source.

## Acceptance criteria

- [ ] Given msautotest's `misc/dateline.map`, when it is rendered, then it is in scope and holds its
  outcome by the metric of G-6, natively and on wasm32 (G-9).
- [ ] Given a line whose reprojected vertices jump across the dateline, when it is drawn, then it
  is wrapped where MapServer wraps it, the expected vertices taken from MapServer's code at the
  pinned tag.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/proj`, `crates/engine/src/lib.rs`, `crates/cli/tests/conformance/cases.txt`
- MapServer's `msTestNeedWrap`, `msProjectShapeEx`, and the `+over` branch of `msProjectRect`
  (`src/mapproject.c`) at the pinned tag
- Seam: the msautotest harness; a test of `crates/proj`

## Blocked by

- docs/plan/reprojection/03-search-rect-as-mapserver.md

## Mode

with the human: its scope depends on how much of `+over` `proj` ports, which may need a decision.
