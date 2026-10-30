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

**A hatch over a polygon.** A polygon layer's style whose symbol is a hatch fills the polygon
with parallel lines, as the hatch branch of `msDrawShadeSymbol` in `src/maprendering.c` and
`msHatchPolygon` and `createHatch` in `src/mapagg.cpp` draw it, in the `COLOR`; without one, the
polygon's outline is drawn as a line in the `OUTLINECOLOR`, as for any polygon style. The lines
stand `SIZE` apart, 1 without one, bounded by `MAXSIZE` and then `MINSIZE`; they are `WIDTH`
wide, 1 without one or for one not above 0, bounded by `MAXWIDTH` and then `MINWIDTH`; they turn
counter-clockwise from the horizontal by `ANGLE`, and are counted from the pixel of the map's
origin, so that the hatches of neighbouring polygons line up. The polygon is the feature's whole
shape, all of its parts at once; where it reaches beyond the map's extent, it is first clipped,
as `msDrawShape` in `src/mapdraw.c` clips it with `msClipPolygonRect` in `src/mapprimitive.cpp`,
to the extent grown by the class's clip buffer: per style, the largest of its symbol's default
size, its `SIZE`, and its `WIDTH`, grown by its larger `OFFSET` unless that is a line offset, or
its `MINSIZE` or `MINWIDTH` where larger, plus 1, rounded half to even; the largest of the
styles, plus 2. It is clipped in map units, or, where a style of the class has a `GEOMTRANSFORM`,
in pixels after the transformation, to the image so grown. Each line runs across the clipped
polygon's bounding box grown by 0.7072 widths, so that the box sets where its dashes fall, is
dashed by `PATTERN` from its first point, the lengths scaled by the factor the bounds applied to
the width, and is stroked with butt caps. The stroke is cut from the polygon as MapServer's
Clipper cuts it ([ADR-0032](../adr/0032-hatch-cut-by-i-overlay.md)): the intersection of the
polygon with the stroke's rectangles, both read under the even-odd rule, every coordinate rounded
to a hundredth of a pixel; so where stroked lines overlap, at a `SIZE` below the `WIDTH`, and
where the polygon's rings overlap, the hatch is empty. A `WIDTH` of 0 makes MapServer divide the
pattern by 0, and AGG draws its infinite dashes as solid lines. `OFFSET` moves the polygon after
the clip. The cut is filled under the non-zero rule through the AGG output format's gamma
([`style-symbol.md`](style-symbol.md)).

A hatch that would not end or that costs past its bounds is not drawn, and the layer warns with
`hatch-not-drawn` (Q-9): a spacing not above 0; a pattern with a length below 0 or not finite,
or whose lengths add up to 0; a spacing that no longer moves the lines where they cross the
image, so far from the map's origin; more than 1 048 576 lines, 2^23 pixels of lines, or 2^20
dashes and gaps over the image; a polygon of more than 16 384 vertices; or more than 2^21
crossings of its edges with the stroke, as estimated from the bands of the lines each edge
reaches across. MapServer loops forever on the first three and for as long as it takes on the
rest. Only the lines that cross the image are laid; the others draw nothing.

**Not yet as MapServer draws a hatch.** A size, width, or angle bound to a property or given by
an expression is drawn as if unset, and the scale factor is 1; both count so in the clip buffer
too, where MapServer counts the values bound for the feature drawn before, since it computes the
buffer before it binds the feature's. A pixmap or SVG symbol of the class counts in the clip
buffer at a default size of 1, where MapServer takes its image's height. i_overlay snaps points
that come within a hundredth of a pixel of an edge, which Clipper does not. The polygon of any
other style is not clipped, which draws the same.

**Not yet as MapServer draws it at a point.** A point style without a symbol it draws — none, 0,
a pixmap, TrueType, SVG, hatch, or simple symbol, a symbol bound to a property, or a name no
symbol carries — draws a square of its size, where MapServer draws nothing for none and 0 and the
symbol or image otherwise. `OFFSET` and `POLAROFFSET` do not move a symbol at a point. The scale
factor is 1: `RESOLUTION`, `SIZEUNITS`, and `SYMBOLSCALEDENOM` do not scale a symbol yet
([`scale.md`](scale.md)). A size, angle, or width bound to a property or given by an expression
is drawn as if unset.
