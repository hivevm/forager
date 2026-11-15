# Ticket: TrueType symbols anchored, along a line, and as a polygon fill

## What to build

A TrueType symbol is drawn everywhere MapServer draws one. The marker of ticket 05 gets its size and
its anchor point. It is also placed along a line with its gap, and tiled as a polygon's fill. Each
use plugs the glyph marker of ticket 05 into a path that already exists for pixmap and vector
symbols, which keeps this to one session.

- **Size and anchor.** `msGetMarkerSize` (`src/mapsymbol.c:759-768`) sizes a TrueType symbol by its
  character's bounds from `msGetCharacterSize` (`:693`). `marker_size_with` in
  `crates/symbolize/src/symbol.rs` reports that size as not computed yet. `msAdjustMarkerPos`
  (`src/maprendering.c:1013`) then shifts the marker by its `ANCHORPOINT`. Case:
  `renderers/marker_anchorpoint.map`.
- **Along a line.** `msImagePolylineMarkers` (`src/maprendering.c:388`, the TrueType branches at
  `:399` and `:489`) spaces the glyphs by its own width and the gap, `INITIALGAP` included. Case:
  `renderers/line_marker_truetype.map`.
- **As a polygon fill.** `getTile` (`src/maprendering.c:222`, the TrueType branch at `:244`) draws
  the glyph centred into a tile, which `msDrawShadeSymbol` (`src/maprendering.c:733`) tiles with its
  `GAP` and `ANGLE`. Case: `renderers/polygon-ttf.map`. The tile is held to Q-9's bound on a fill
  tile and counted with the render's images (ADR-0029), as a pixmap's tile is.

## Acceptance criteria

- [ ] Given `renderers/marker_anchorpoint.map`, `renderers/line_marker_truetype.map`, and
  `renderers/polygon-ttf.map`, when the harness renders each, then it passes by the metric of G-6,
  natively and bit-identically on wasm32 (G-9), and the list says `geometry passes`.
- [ ] Given a TrueType symbol, when its marker size is asked for, then it is its character's bounds
  as `msGetCharacterSize` gives them at the style's size. The engine no longer reports the marker
  size of a TrueType symbol as not computed.
- [ ] Given a TrueType fill whose tile passes Q-9's bound on a fill tile, when it is drawn, then the
  engine reports the bound's diagnostic and draws no fill, as for a pixmap's tile.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/symbolize/src/symbol.rs` (`marker_size_with`, `anchor_shift`),
  `crates/symbolize/src/line_marker.rs`, `crates/symbolize/src/shade.rs`,
  `crates/symbolize/src/tile.rs`, `crates/render/src/tile.rs`,
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapsymbol.c` (`msGetMarkerSize`, `msGetCharacterSize` at `:693`),
  `src/maprendering.c` (`getTile`, `msImagePolylineMarkers`, `msDrawShadeSymbol`,
  `msAdjustMarkerPos`), `src/fontcache.c`
- `docs/semantics/symbol.md`, `docs/semantics/style-symbol.md`
- `docs/adr/0029-pixmaps-held-once-per-render.md`, `docs/adr/0031-label-glyphs.md`
- Golden path: the pixmap's tile and line markers, and their tests under `crates/symbolize/src/`
- Seam: the msautotest harness; the symbolizer's unit tests

## Blocked by

- docs/plan/fidelity/05-truetype-symbol-at-a-point.md

## Mode

unattended:

- The harness decides each criterion.
- Each use is isolated in the symbolizer, behind the glyph marker the tracer proved. A wrong result
  costs one revert.
- It touches no part ADR-0006 keeps with the human. It uses Q-9's bound on a fill tile and does not
  change it.
- A wrong result shows as a case that fails or a case in scope that changes outcome. The harness's
  outcomes and the difference images prove the result.
- Stop: when a case still fails after two attempts at its remaining difference, or when a change to
  a resource limit or the style document looks needed, the session commits the cases that pass and
  hands off with the rest. If two of the three cases are done and the session is past half its
  context, it hands off and leaves the third as a ticket.
