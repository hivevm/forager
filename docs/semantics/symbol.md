# `SYMBOLSET` and `SYMBOL`

**Where:** `SYMBOLSET` in `MAP`, naming a symbol file; `SYMBOL` blocks in `MAP` and in the symbol
file. **IR:** the map's `symbols`.

**Meaning.** The symbols the styles draw. The frontend reads the symbol file through the resource
loader, resolved against the main mapfile's directory inside the base context (Q-8), as
`loadSymbolSet` in `src/mapsymbol.c` resolves it against the map's path; it reads it within the
limits of `INCLUDE` (Q-9), and it must start with `SYMBOLSET`. Of several `SYMBOLSET`s in the map,
the last names the file. `loadSymbolSet` counts `SYMBOLSET` and `END` levels and reads to the end
of the file, so a set nested in the set, and a set that follows it, add their symbols too.

**Order and index.** MapServer numbers its symbols: 0 is its default symbol, a filled circle of
size 1; then come the map's `SYMBOL` blocks in the order they stand, which `loadMap` appends as it
parses them, then the symbol file's in the order they stand, which it appends after the whole map.
The IR lists them in that order without the default, so the symbol at position `i` has index `i +
1`.

**A symbol**, as `loadSymbol` reads it: `NAME`; `TYPE` `vector` (the default), `ellipse`, `pixmap`,
`simple`, `truetype`, `hatch`, or `svg`; `FILLED`; `ANCHORPOINT`, two numbers from 0 to 1, default
the centre; `CHARACTER` and `FONT` for a TrueType symbol; `IMAGE` for a pixmap or SVG, a resource
path resolved beside the file that holds the symbol — the mapfile or the symbol file — and not read
at load (Q-8); `TRANSPARENT`, a palette index from 0 to 255; `POINTS`, at most 100 pairs in all, as
a second `POINTS` block appends to the first. `TYPE` and `FILLED` are bare words, `TRUE` and `ON`,
`FALSE` and `OFF` alike for `FILLED`; a later statement replaces an earlier one. A pixmap or SVG
without `IMAGE`, and a vector or ellipse without `POINTS`, are load errors. `ANTIALIAS` is read and
ignored, as MapServer ignores it.

**Vector symbols** are measured and shifted as `loadSymbol` does at its `END`: the extent is the
largest x and y, from 0, of the points of the last `POINTS` block whose x is not -99; a pair
`-99 -99` lifts the pen and starts a new line. The smallest x and y are taken from the first point
and the points with neither coordinate -99; where one is negative, or the anchor point is not the
centre and one is not 0, those points, and the extent, are shifted by them. A point with one
coordinate -99 is drawn where it stands. A style's size is the symbol's height drawn
(`msSymbolGetDefaultSize`: its extent's height, or its width where the height is 0), so a symbol 2
wide and 4 high drawn at size 8 is 4 wide and 8 high. An ellipse is measured the same way and never
shifted; its extent is its width and height.

**Drawing at a point.** A point layer's style draws its vector or ellipse symbol at each point
inside the map's extent, as `msDrawMarkerSymbol` and `computeSymbolStyle` in
`src/maprendering.c` and the AGG renderer's `agg2RenderVectorSymbol` and
`agg2RenderEllipseSymbol` in `src/mapagg.cpp` draw it. Its scale is the style's size, bounded by
`MINSIZE` and `MAXSIZE`, over its default size; without a size it is drawn at its own. A filled
symbol is filled with `COLOR`, its lines closed, and outlined with `OUTLINECOLOR`; one not
filled is only outlined, with `COLOR`, or `OUTLINECOLOR` where there is none. The outline is the
style's `WIDTH`, 1 by default, with AGG's butt caps and miter joins; a vector symbol's lines stay
open, an ellipse's closes. A vector symbol is drawn about the centre of its extent, an ellipse is
AGG's polygon of its extent scaled; both turn counter-clockwise by `ANGLE`. An anchor point other
than the centre moves the symbol so that the anchor, turned with it, lies on the point, measured
at the size cut to whole pixels (`msAdjustMarkerPos`, `msGetMarkerSize` in `src/mapsymbol.c`).
A style whose symbol index lies past the last symbol draws nothing.
