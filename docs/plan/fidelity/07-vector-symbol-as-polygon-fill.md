# Ticket: A vector symbol tiled as a polygon's fill

## What to build

A polygon styled with a vector symbol is filled with the symbol tiled as MapServer tiles it, at the
style's size, gap, and angle, natively and on wasm32. MapServer's `msDrawShadeSymbol`
(`src/maprendering.c:733`) asks `getTile` (`src/maprendering.c:222`) for a tile. For a vector
symbol, `getTile` renders it centred into the tile with `renderVectorSymbol`
(`agg2RenderVectorSymbol`, `src/mapagg.cpp:612`), turned and spaced by the style.
`agg2RenderPolygonTiled` (`src/mapagg.cpp:529`) then fills the polygon with the tile. Forager
already fills a polygon with a pixmap's tile, held to Q-9's bound on a fill tile and counted with
the render's images (ADR-0029). The vector symbol reuses that tile path.

Cases: `renderers/polygon-vector.map` (the symbol `vector arrow` at sizes 8 and 16, `GAP 0`, and
`ANGLE 40`) and `renderers/polygon-as-line.map` (the same symbol as a fill, under an outline). Both
note today that they lack the tiled vector symbol.

## Acceptance criteria

- [ ] Given `renderers/polygon-vector.map` and `renderers/polygon-as-line.map`, when the harness
  renders each, then it passes by the metric of G-6, natively and bit-identically on wasm32 (G-9),
  and the list says `geometry passes`.
- [ ] Given a vector symbol as a fill, when its tile is built, then its size and the symbol's place
  in it are those `getTile` computes for the style's size, gap, and angle. A unit test of the
  symbolizer holds the numbers taken from the C at the pin.
- [ ] Given a vector fill whose tile passes Q-9's bound on a fill tile, when it is drawn, then the
  engine reports the bound's diagnostic and draws no fill.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/symbolize/src/shade.rs`, `crates/symbolize/src/tile.rs`,
  `crates/symbolize/src/symbol.rs` (the vector symbol's shape), `crates/render/src/tile.rs`,
  `crates/render-agg/src/lib.rs` (the tiled fill), `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/maprendering.c` (`getTile`, `msDrawShadeSymbol`), `src/mapagg.cpp`
  (`agg2RenderVectorSymbol`, `agg2RenderPolygonTiled`)
- Glossary: Fill tile
- `docs/semantics/symbol.md`, `docs/semantics/style-symbol.md`
- `docs/adr/0029-pixmaps-held-once-per-render.md`, `docs/adr/0026-agg-blender-and-resampler.md`
- Golden path: the pixmap's fill tile in `crates/symbolize/src/tile.rs` and
  `crates/symbolize/src/shade.rs` with their tests
- Seam: the msautotest harness; the symbolizer's unit tests

## Blocked by

- None (can start immediately)

## Mode

unattended:

- The harness and the symbolizer's tests decide each criterion.
- It is isolated in the fill path of the symbolizer and the renderer. A wrong result costs one
  revert.
- It touches no part ADR-0006 keeps with the human. It uses Q-9's bound on a fill tile and does not
  change it.
- A wrong result shows as a case that fails or a case in scope that changes outcome. The harness's
  outcomes and the difference images prove the result.
- Stop: when a case still fails after two attempts at its remaining difference, or when a change to
  a resource limit or the style document looks needed, the session commits what passes and hands
  off.
