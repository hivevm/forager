# API-truetype-marker-through-port: A TrueType symbol at a point is drawn from a glyph symbolize asks of a port, which the engine answers through label

- **Status:** 🟢 accepted
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the TrueType marker of `crates/symbolize` and the glyph port it defines, the engine's answer to that port in `crates/engine` with its fonts, and the reading of a single glyph in `crates/label`

## Context

A point styled with a TrueType symbol draws nothing in Forager today. A case like
`renderers/point_truetype.map` fails by the metric of G-6. Fidelity ticket 05 is the tracer
bullet: a glyph drawn as a marker, natively and bit-identically on wasm32 (G-9).

MapServer at the pin, `rel-8-6-6`, draws it in the TrueType branch of
`msDrawMarkerSymbolInternal` (`src/maprendering.c`):

1. It looks up the face by the symbol's `FONT` in the font set (`msGetFontFace`) and fails the
   symbol where there is none.
2. It maps the first character of `CHARACTER` (`msUTF8ToUniChar`) to a glyph index
   (`msGetGlyphIndex`, `FT_Get_Char_Index`). A character the font lacks maps to glyph 0, which
   loads.
3. It takes the glyph unhinted at `MS_MAX(MS_NINT(s.scale), 1)` points, which is
   `MS_NINT(size * 96 / 72)` pixels (`msGetGlyphByIndex`, `src/fontcache.c`). `s.scale` is
   computeSymbolStyle's target size: a TrueType symbol's default size is 1, so it is the `SIZE`
   times the scale factor, held to `MINSIZE` and `MAXSIZE`.
4. `drawGlyphMarker` centres the glyph by its metrics. The centre is
   `((maxx + minx) / 2, (maxy + miny) / 2)`, turned by the rotation where there is one, and the
   glyph's origin is the point minus that centre in x and plus it in y.
5. `agg2RenderGlyphsPath` (`src/mapagg.cpp`) draws it, the renderer labels use: the outline first,
   as `conv_contour` of width `outlinewidth + 1`, the width truncated to `int`; then the fill,
   non-zero.

The forces:

- **[REN-label-glyphs-from-skrifa](REN-label-glyphs-from-skrifa.md)** decides how a *label's* glyphs
  are read, laid out, and drawn. Its sizes, its `FONT` list, its outline width, and its loading of
  fonts "when a label first needs it" are a label's. A glyph drawn as a marker has other sizes, one
  font, and another outline width, so REN-label-glyphs-from-skrifa does not cover it. It also binds
  skrifa to `label` alone.
- **[ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md)** lets `symbolize`
  depend on the IR, `expr`, `proj`, and `render`, not on `label`. The symbolizer places and draws
  markers, so the glyph has to reach it some other way.
- A style's `SIZE` can be bound to an attribute, so the glyph's size is known only per feature, as
  the symbolizer draws it.
- The display list already carries glyph outlines (`DrawOp::Glyphs`), and `render-agg` draws them
  as `agg2RenderGlyphsPath` does.
- The maintainer chose two things on 2026-11-15: the port, over a dependency on `label`; and
  glyph 0 with a warning for a missing character.

## Decision

We will draw a TrueType symbol at a point from a glyph that `symbolize` asks of a port of its own,
which the engine answers through `label`. The glyph is placed and drawn as `drawGlyphMarker` and
`agg2RenderGlyphsPath` place and draw it. The same glyph is anchored, placed along a line, and
tiled as a polygon's fill.

What binds:

- **The port.** `symbolize` defines a trait that takes a font's name, a character, and a glyph
  size in points. It returns the glyph's outline in pixels about its origin, y down as the display
  list has it, and its metrics. Where there is none, it returns why. The canvas carries the port,
  as it carries the render's pixmaps. The engine's label cache also asks the port, through
  `marker_size_of` and `marker_size_with`. `symbolize` gains no dependency.
- **The engine's answer.** The engine looks up the font's path in the font set. It loads the font
  through the resource loader, once per render, into the store the labels' fonts use. `label`
  reads the glyph. `label` reads it as it reads a label's glyph: unhinted, at the pixel size
  `MS_NINT(size * 96 / 72)`, from face 0 at its default instance, through its Unicode character
  map. skrifa stays in `label` alone.
- **The size.** MapServer reads the glyph at two sizes, and Forager asks the port at both:
  - The drawn glyph is asked at `MS_MAX(MS_NINT(scale), 1)` points. The scale is
    computeSymbolStyle's for a symbol of default size 1.
  - The marker's size is the glyph's control box at msGetMarkerSize's size. That size is an `int`:
    the style's size, 1 without one, times the scale factor, held to `MINSIZE` and `MAXSIZE`
    unscaled. The anchor shift and the label cache's marker size use it.
- **The place.** The glyph's origin is the point shifted by the centre of its metrics, as
  `drawGlyphMarker` computes it. Where the style turns it, the centre is turned with `libm`'s sine
  and cosine (Q-2) and the glyph turned by the same angle.
- **The drawing.** One `DrawOp::Glyphs` per marker. Its outline is the `OUTLINECOLOR` at a width
  of the outline width truncated to an integer, plus 1, where the style has one. Its fill is the
  `COLOR`.
- **A missing character.** Where the font lacks the character, glyph 0 is drawn, as MapServer
  draws it. The layer warns with `symbol-glyph-missing`, once per symbol, naming the symbol, the
  character, and the font.
- **A font that cannot be used.** This covers a `FONT` the font set does not name, a file that does
  not load, and one REN-label-glyphs-from-skrifa calls unusable. Nothing is drawn for that style.
  The layer warns with `symbol-not-drawn`, once per symbol, naming the symbol, the font, and its
  path.
- **The anchor.** An `ANCHORPOINT` other than the centre moves the marker as `msAdjustMarkerPos`
  moves it, by the marker's size.
- **Along a line.** A style with a `GAP` other than 0 places the glyphs through the line markers
  that place other symbols, as `msImagePolylineMarkers` places them. The marker's size is the drawn
  glyph's control box, not held to 1 pixel.
- **As a polygon's fill.** A style with a `COLOR` fills the polygon with the glyph tiled, as
  `getTile` draws it. The tile goes through the render's tile cache and is never seamless. Q-9's
  bound on a fill tile holds it. Where the bound refuses the tile, nothing is drawn, and the layer
  warns with `symbol-not-drawn`.

**Out of scope:**

- a TrueType symbol along a line with a `GAP` of 0, which MapServer tiles along the line;
- a TrueType symbol as a label's marker;
- fonts read through a Symbol character map (`msGetGlyphIndex`'s `| 0xf000`), and CFF outlines, as
  in REN-label-glyphs-from-skrifa.

## Alternatives considered

- **`symbolize` depends on `label`.** Simpler code: the symbolizer reads the glyph itself. But it
  adds an edge ARCH-crates-as-ports-and-adapters's table refuses, and that table is accepted. So it
  would supersede ARCH-crates-as-ports-and-adapters, for one function. The maintainer chose the
  port.
- **The engine draws the glyph and hands the operation in.** The engine knows the fonts. But the
  size and the angle can be bound per feature, and the place depends on the symbolizer's point. The
  engine would repeat the symbolizer's work per feature.
- **`symbolize` reads fonts with skrifa itself.** No port, but it breaks
  REN-label-glyphs-from-skrifa's rule that skrifa is `label`'s alone, and splits glyph reading in
  two.
- **A missing character draws nothing, with a diagnostic.** The ticket assumed this was
  MapServer's behaviour. MapServer draws glyph 0, so this would deviate from the reference.
- **A missing character draws glyph 0 silently.** Exactly MapServer, but a mistyped `CHARACTER`
  would pass unseen. The maintainer chose the warning.

## Sources / Prior art

- MapServer `rel-8-6-6`:
  - `src/maprendering.c`: computeSymbolStyle, `drawGlyphMarker`, and the TrueType branch of
    `msDrawMarkerSymbolInternal`;
  - `src/fontcache.c`: `msGetGlyphIndex` and `msGetGlyphByIndex`;
  - `src/mapagg.cpp`: `agg2RenderGlyphsPath`;
  - `src/maprendering.c`: `msAdjustMarkerPos`, `msImagePolylineMarkers`, `getTile`, and
    `msDrawShadeSymbol`;
  - `src/mapsymbol.c`: `msSymbolGetDefaultSize`, `msGetMarkerSize`, and `msGetCharacterSize`.
- [REN-label-glyphs-from-skrifa](REN-label-glyphs-from-skrifa.md) and
  [ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md).
- Fidelity tickets 05 and 06, and the cases `renderers/point_truetype.map`,
  `renderers/marker_anchorpoint.map`, `renderers/line_marker_truetype.map`, and
  `renderers/polygon-ttf.map` of msautotest at the pin.

## Consequences

- Positive:
  - A TrueType symbol at a point, anchored, along a line, and as a fill is drawn as MapServer
    draws it (G-6, G-9).
  - The glyph comes from the one place that reads glyphs, and no crate edge changes.
- Negative / trade-offs:
  - The canvas carries one more handle.
  - The engine answers per glyph asked. A cache by font, character, and size may be needed where
    many markers draw one glyph, as MapServer's glyph cache has.
  - `symbol-glyph-missing` is a new public diagnostic code.
- Follow-ups:
  - a TrueType symbol as a label's marker;
  - along a line with a `GAP` of 0.

## Enforcement

- A unit test of the symbolizer cites this ADR. It holds the glyph's origin for a point, a centre,
  and an angle, with the numbers `drawGlyphMarker` computes.
- An engine test cites this ADR. It holds `symbol-not-drawn` for a `FONT` the font set lacks, and
  `symbol-glyph-missing` with glyph 0 drawn for a character the font lacks.
- The conformance list holds these cases as `geometry passes`:
  - `renderers/point_truetype.map point_truetype.png`;
  - `renderers/marker_anchorpoint.map marker_anchorpoint.png`;
  - `renderers/line_marker_truetype.map line_marker_truetype.png`;
  - `renderers/polygon-ttf.map polygon-ttf.png`.
- An engine test cites Q-9. It holds `symbol-not-drawn`, and no fill, for a TrueType fill whose
  tile passes the bound.
- The arch tests already hold the rest:
  - `symbolize` has no edge to `label` (ARCH-crates-as-ports-and-adapters);
  - only `label` reads fonts with skrifa (REN-label-glyphs-from-skrifa).
