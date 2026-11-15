# Ticket: A TrueType symbol at a point (tracer bullet)

## What to build

A point styled with a TrueType symbol shows the symbol's glyph where MapServer draws it, at its
size, colour, and angle, natively and on wasm32. This is the tracer bullet through new ground. It
carries a glyph from the font set through the symbolizer to the renderer as a marker, not as a
label. Ticket 06 builds the other uses of a TrueType symbol on it.

MapServer draws the marker in `msDrawMarkerSymbolInternal` (`src/maprendering.c:965-979`). It looks
up the face by the symbol's `FONT` in the font set, takes the glyph of its `CHARACTER` at
`MS_MAX(MS_NINT(s.scale), 1)`, and hands it to `drawGlyphMarker` (`src/maprendering.c:183-221`).
`drawGlyphMarker` centres the glyph on the point by its metrics, turns the centre by the rotation,
and calls the renderer's `renderGlyphs`, which is `agg2RenderGlyphsPath` (`src/mapagg.cpp:550`). The
label path already reads skrifa's outlines and fills them as `agg2RenderGlyphsPath` does (ADR-0031).
The session reuses that path and its glyph display item, rather than building a second path. The
engine's fonts are loaded today for the fonts a render's labels name. A TrueType symbol's font joins
them through the same resource loader and the same per-render store.

The case is `renderers/point_truetype.map point_truetype.png`: the glyph `A` of the font `default`,
plain and at `ANGLE 45`, beside circle anchors.

## Acceptance criteria

- [ ] Given `renderers/point_truetype.map`, when the harness renders `point_truetype.png`, then it
  passes by the metric of G-6, natively and bit-identically on wasm32 (G-9), and the list says
  `geometry passes`.
- [ ] Given a TrueType symbol at a point, when the symbolizer places it, then the glyph's origin is
  the point shifted by half its metrics and turned by the style's angle, as `drawGlyphMarker`
  computes it. A unit test of the symbolizer holds the numbers taken from the C at the pin.
- [ ] Given a TrueType symbol whose font the font set does not name, or whose character the font
  lacks, when it is drawn, then the engine reports a diagnostic naming the symbol and draws nothing
  for it, as MapServer fails the symbol.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/symbolize/src/marker.rs`, `crates/symbolize/src/symbol.rs` (the marker of a
  TrueType symbol), `crates/engine/src/fonts.rs`, `crates/engine/src/lib.rs` (where a style's fonts
  are gathered), `crates/render/src/lib.rs` (the `Glyphs` item), `crates/render-agg/src/glyphs.rs`,
  `crates/cli/tests/conformance/cases.txt`
- `crates/label/src/font.rs` (how a glyph's outline and metrics are read),
  `crates/style-ir/src/model.rs` (the symbol's glyph and font)
- MapServer at the pin: `src/maprendering.c` (`drawGlyphMarker`, the TrueType branch of
  `msDrawMarkerSymbolInternal`), `src/mapagg.cpp` (`agg2RenderGlyphsPath`), `src/fontcache.c`
  (`msGetGlyphByIndex`)
- `docs/semantics/symbol.md`, `docs/semantics/angle.md`
- `docs/adr/0031-label-glyphs.md` (its *Decision* and *Applies to*: whether a glyph drawn as a
  marker falls under it)
- Golden path: the marker of a pixmap symbol in `crates/symbolize/src/marker.rs` and its tests in
  `crates/symbolize/src/marker/tests.rs`
- Seam: the msautotest harness; the symbolizer's unit tests; the engine's render with sources in
  memory (`crates/engine/tests/fonts.rs`)

## Blocked by

- None (can start immediately)

## Mode

with the human: it is the tracer bullet through new ground, and it may find that ADR-0031 does not
cover a glyph drawn as a marker. In that case the session stops and proposes an ADR before going on.
It also stops when the case still fails after two attempts at its remaining difference, and hands
off with the share of differing pixels and the difference image.
