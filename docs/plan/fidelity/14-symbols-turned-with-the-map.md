# Ticket: Symbols and labels turned with a rotated map

## What to build

On a rotated map, a style's or a label's angle is turned with the map, as MapServer turns it. In
`pointLayerDrawShape` (`src/mapdraw.c:2034`, the turn at `:2050-2060`, undone at `:2112-2120`),
MapServer subtracts the map's rotation from every style and label angle of a point layer that is not
0 before it draws, and adds it back after. `lineLayerDrawShape` (`:2127`) and
`polygonLayerDrawShape` (`:2283`) turn the labels' angles alone. An angle of exactly 0 is left as it
is. So a symbol of `ANGLE 0` and one at the map's own angle both draw upright on the image, while
the shapes are rotated. A line's `PATTERN` is drawn along the rotated line.

Cases: `renderers/symbolrot.map` and `renderers/symbolrot_negative.map` (both `symbolrot.png`):
`ANGLE 45`, a dashed line, and point layers whose vector symbol `v-line` has angles equal to, close
to, and away from the map's.

**The harness's projection for these cases is a stand-in.** The map and the layers are in
`init=epsg:21781`, the Swiss oblique Mercator. The suite's `wxs/data/epsg2` lacks that code, so the
msautotest harness gives it a stand-in in longitude and latitude (`EPSG_CODES` in
`crates/cli/tests/msautotest.rs`). The coordinates, such as `530000 151000`, are not degrees. On a
rotated map every layer reprojects (ticket 13). MapServer reprojects through PROJ's `somerc`, which
goes forward and back to nearly the same point. Forager would reproject through the
longitude-latitude stand-in, which may fail or wrap. The session chooses one way, and says which in
the hand-off:

- ticket 13's rotation leaves the coordinates as they are where the layer's and the map's
  projections are the same proj4 string, applying only the geotransform. This matches MapServer to
  PROJ's round-trip error, and the ticket says so;
- `proj` implements `somerc`, and the harness gets the real definition of 21781 (ADR-0037 allows a
  projection a case needs). That makes this ticket two sessions; split it.

## Acceptance criteria

- [ ] Given `renderers/symbolrot.map` and `renderers/symbolrot_negative.map`, when the harness
  renders each, then it passes by the metric of G-6, natively and bit-identically on wasm32 (G-9),
  and the list says `geometry passes`.
- [ ] Given a rotated map, when a point layer's style of angle `a` that is not 0 is drawn, then its
  symbol is turned by `a` minus the map's rotation. A style of angle 0 is not turned. A test of the
  engine's render with sources in memory decides it.
- [ ] Given a rotated map, when a label of an angle that is not 0 is placed, then its angle is
  turned as the layer's draw function at the pin turns it, and the label cache tests it at that
  angle.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/engine/src/lib.rs` (where a style's angle is evaluated per feature),
  `crates/engine/src/label_binding.rs`, `crates/symbolize/src/marker.rs`,
  `crates/symbolize/src/style.rs`, `crates/engine/src/reproject.rs` (if the same-projection path is
  chosen), `crates/cli/tests/msautotest.rs` (`EPSG_CODES`, if `somerc` is chosen),
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapdraw.c` (`pointLayerDrawShape`, `lineLayerDrawShape`,
  `polygonLayerDrawShape`), `src/mapproject.c` (the `gt` branches)
- `docs/semantics/angle.md`, `docs/semantics/label.md`
- `docs/adr/0037-reprojection-ported-in-proj.md`, `docs/adr/0032-label-cache.md`, and the ADR ticket
  13 proposed, if any
- If `somerc` is chosen: `docs/adr/0038-proj-references-from-mapserver.md` (projection
  references) and `docs/adr/0019-projection-resolver.md` (the resolver)
- Golden path: the style's `ANGLE` in `crates/symbolize/src/marker.rs` and its tests
- Seam: the msautotest harness; the engine's render with sources in memory

## Blocked by

- docs/plan/fidelity/13-map-rotation.md

## Mode

with the human: the choice between an identity for equal projections and a port of `somerc` decides
how the rotation meets ADR-0037's reprojection, and it may split the ticket. The session stops after
it has measured which way MapServer's output is reproduced. It hands the choice to the maintainer
before building. It also stops when a case still fails after two attempts at its remaining
difference.
