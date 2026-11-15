# ADR-draft-truetype-marker-glyph-through-a-port: A TrueType symbol at a point is drawn from a glyph symbolize asks of a port, which the engine answers through label

- **Status:** 🟡 proposed
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

- **[ADR-0031](0031-label-glyphs.md)** decides how a *label's* glyphs are read, laid out, and
  drawn. Its sizes, its `FONT` list, its outline width, and its loading of fonts "when a label
  first needs it" are a label's. A glyph drawn as a marker has other sizes, one font, and another
  outline width, so ADR-0031 does not cover it. It also binds skrifa to `label` alone.
- **[ADR-0008](0008-ports-and-adapters-crates.md)** lets `symbolize` depend on the IR,
  `expr`, `proj`, and `render`, not on `label`. The symbolizer places and draws markers, so the
  glyph has to reach it some other way.
- A style's `SIZE` can be bound to an attribute, so the glyph's size is known only per feature, as
  the symbolizer draws it.
- The display list already carries glyph outlines (`DrawOp::Glyphs`), and `render-agg` draws them
  as `agg2RenderGlyphsPath` does.
- The maintainer chose two things on 2026-11-15: the port, over a dependency on `label`; and
  glyph 0 with a warning for a missing character.

## Decision

We will draw a TrueType symbol at a point from a glyph that `symbolize` asks of a port of its own,
which the engine answers through `label`. The glyph is placed and drawn as `drawGlyphMarker` and
`agg2RenderGlyphsPath` place and draw it.

What binds:

- **The port.** `symbolize` defines a trait that takes a font's name, a character, and a glyph
  size in points. It returns the glyph's outline in pixels about its origin, y down as the display
  list has it, and its metrics. Where there is none, it returns why. The canvas carries the port,
  as it carries the render's pixmaps. `symbolize` gains no dependency.
- **The engine's answer.** The engine looks up the font's path in the font set. It loads the font
  through the resource loader, once per render, into the store the labels' fonts use. `label`
  reads the glyph. `label` reads it as it reads a label's glyph: unhinted, at the pixel size
  `MS_NINT(size * 96 / 72)`, from face 0 at its default instance, through its Unicode character
  map. skrifa stays in `label` alone.
- **The size.** The glyph is asked at `MS_MAX(MS_NINT(scale), 1)` points. The scale is
  computeSymbolStyle's for a symbol of default size 1.
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
  not load, and one ADR-0031 calls unusable. Nothing is drawn for that style. The layer warns with
  `symbol-not-drawn`, once per symbol, naming the symbol, the font, and its path.

**Out of scope:**

- a TrueType symbol along a line, anchored to it, or as a polygon's fill;
- a TrueType symbol as a label's marker;
- its `ANCHORPOINT`;
- fonts read through a Symbol character map (`msGetGlyphIndex`'s `| 0xf000`), and CFF outlines, as
  in ADR-0031.

## Alternatives considered

- **`symbolize` depends on `label`.** Simpler code: the symbolizer reads the glyph itself. But it
  adds an edge ADR-0008's table refuses, and that table is accepted. So it would supersede
  ADR-0008, for one function. The maintainer chose the port.
- **The engine draws the glyph and hands the operation in.** The engine knows the fonts. But the
  size and the angle can be bound per feature, and the place depends on the symbolizer's point.
  The engine would repeat the symbolizer's work per feature.
- **`symbolize` reads fonts with skrifa itself.** No port, but it breaks ADR-0031's rule that
  skrifa is `label`'s alone, and splits glyph reading in two.
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
  - `src/mapsymbol.c`: `msSymbolGetDefaultSize`.
- [ADR-0031](0031-label-glyphs.md) and [ADR-0008](0008-ports-and-adapters-crates.md).
- Fidelity ticket 05, `docs/plan/fidelity/05-truetype-symbol-at-a-point.md`, and the case
  `renderers/point_truetype.map` of msautotest at the pin.

## Consequences

- Positive:
  - A TrueType symbol at a point is drawn as MapServer draws it (G-6, G-9).
  - The glyph comes from the one place that reads glyphs, and no crate edge changes.
  - Ticket 06 (TrueType along lines and as fills) can ask the same port.
- Negative / trade-offs:
  - The canvas carries one more handle.
  - The engine answers per glyph asked. A cache by font, character, and size may be needed where
    many markers draw one glyph, as MapServer's glyph cache has.
  - `symbol-glyph-missing` is a new public diagnostic code.
- Follow-ups:
  - TrueType symbols along lines and as fills;
  - as a label's marker;
  - their `ANCHORPOINT`.

## Enforcement

- A unit test of the symbolizer cites this ADR. It holds the glyph's origin for a point, a centre,
  and an angle, with the numbers `drawGlyphMarker` computes.
- An engine test cites this ADR. It holds `symbol-not-drawn` for a `FONT` the font set lacks, and
  `symbol-glyph-missing` with glyph 0 drawn for a character the font lacks.
- The conformance list holds `renderers/point_truetype.map point_truetype.png` as
  `geometry passes`.
- The arch tests already hold the rest:
  - `symbolize` has no edge to `label` (ADR-0008);
  - only `label` reads fonts with skrifa (ADR-0031).
