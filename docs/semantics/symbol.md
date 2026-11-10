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

**A pixmap at a point.** A point layer's style whose symbol is a pixmap draws its image at each
point inside the map's extent, as `msDrawMarkerSymbolInternal` in `src/maprendering.c` and
`agg2RenderPixmapSymbol` in `src/mapagg.cpp` draw it. The image is read through the resource
loader the first time a feature the layer draws names it, in a style within its scale range,
from the path its `IMAGE` names, within 32 MiB, the bound of a font file, and decoded once for
the whole render, which every point draws from
([ADR-0040](../adr/0040-a-decoded-pixmap-is-held-once-per-render.md)); a PNG is read as `readPNG`
in `src/mapimageio.c` reads it, its colours premultiplied by their alpha. The image is read, and
counted against the bytes of the render's images, even where none of the feature's points lies
inside the map's extent; MapServer reads it only when `msDrawMarkerSymbol` runs, so there a pixmap
no point draws is never read, and one that does not load fails nothing. Its scale is the
style's size, its default the image's height, bounded by `MINSIZE` and `MAXSIZE`, over the
image's height; so without a size, and with `MAXSIZE` not below the height, it is 1. The point is
moved by the style's `OFFSET` and `POLAROFFSET` as a symbol's is, and by the symbol's anchor point,
turned with the image, as a vector symbol's (`msAdjustMarkerPos`), at the size `msGetMarkerSize`
gives a pixmap: the style's size, or the image's height without one, times the scale factor, cut to
whole pixels, then bounded by `MINSIZE` and `MAXSIZE` unscaled and cut again; at a size of 1 the
image's own width and height, otherwise that size high and as wide as the image is at that height. A
40 × 20 image is 40 × 20 at `SIZE 1`, 20 × 10 at `SIZE 10.9`, and 60 × 30 without a size at a scale
factor of 1.5. Unturned and at a scale of 1, the image is blended unscaled at the point so moved,
every pixel at full cover, its top-left pixel at `MS_NINT(x - width / 2.)` and
`MS_NINT(y - height / 2.)`: a point at the pixel (50.5, 41.5) puts a 32 × 32 image's top-left
pixel at (34, 26), `lrint` rounding 34.5 and 25.5 half to even. Neither the style's colour nor
its opacity reaches the image (`computeSymbolStyle`). A scale of 0 draws nothing
(`msDrawMarkerSymbolInternal`). At a scale other than 1, or turned by an `ANGLE` whose radians,
`ANGLE × MS_DEG_TO_RAD`, are neither 0 nor `MS_PI × 2` (360 is, exactly, in `double`), the image
is resampled as `agg2RenderPixmapSymbol` resamples it: the square of half-side
`MS_NINT(max(width, height) × scale × 1.415) / 2 + 1`, in integer division, about the point is
filled under the non-zero rule without a gamma, each of its pixels the image sampled bilinearly,
transparent beyond it, at the position the inverse of the image's transformation takes the pixel's
centre to: a translation by minus half the image, a rotation by minus the angle, so
counter-clockwise on the map, the scaling, and a translation to the point, in the order of
operations of AGG's `trans_affine`. The filter and the interpolator are AGG 2.4's, ported
([ADR-0041](../adr/0041-render-agg-resamples-pixmaps-as-agg-2-4.md)). A 32 × 32 image at
`SIZE 55` has the half-side `lrint(77.825) / 2 + 1 = 40`. The square is clipped to the image
grown by its margin, as every path is
([ADR-0039](../adr/0039-the-clip-to-the-image-is-one-module-of-render.md)); MapServer
rasterizes it whole. Where the square reaches more than that margin past the image, each row's
span of full cover then starts at the margin, not at the square's edge, and the interpolator,
which rounds a span's two ends to 256ths of a pixel and steps between them, may sample a pixel
of that row a 256th of a pixel apart from MapServer, a channel off by one at most. Farther still,
the two part more: MapServer's rasterizer has no clip box (`agg2RenderPixmapSymbol` sets none),
so it takes the square's corners in 24.8 fixed point, which overflows past some 8 million pixels,
and its packed scanline (`scanline_p8`) holds a span's position in 16 bits, which wraps on an image
wider than 32,767 pixels. Forager clips first and models neither; a square that large, or an image
that wide, draws as the clip leaves it, not as MapServer would.

**A pixmap that does not load** — its file missing, past 32 MiB, of a format other than PNG, broken,
past 4096 × 4096 pixels by its header, or past what the images one render holds decoded may come
to (Q-9) — leaves its style undrawn, and the layer warns with `pixmap-not-drawn`, once per symbol,
naming the symbol, its path, and why; the rest of the map draws. This deviates on purpose, by the
maintainer's decision, from MapServer: there the symbol fails to load, so the layer fails to draw,
and `msDrawMap` (`src/mapdraw.c`) returns no map at all. The label over such a pixmap, its class's
first style, is left out with `label-not-drawn` ([label.md](label.md)).

**A hatch over a polygon.** A polygon layer's style whose symbol is a hatch fills the polygon
with parallel lines, as the hatch branch of `msDrawShadeSymbol` in `src/maprendering.c` and
`msHatchPolygon` and `createHatch` in `src/mapagg.cpp` draw it, in the `COLOR`; without one, the
polygon's outline is drawn as a line in the `OUTLINECOLOR`, as for any polygon style. The lines
stand `SIZE` apart, 1 without one, bounded by `MAXSIZE` and then `MINSIZE`; they are `WIDTH`
wide, 1 without one or for one not above 0, bounded by `MAXWIDTH` and then `MINWIDTH`, the
spacing, the width, their bounds, and the clip buffer below each scaled ([`scale.md`](scale.md));
they turn counter-clockwise from the horizontal by `ANGLE`, and are counted from the pixel of the
map's origin, so that the hatches of neighbouring polygons line up. The polygon is the feature's
whole shape, all of its parts at once; where it reaches beyond the map's extent, it is first
clipped, as `msDrawShape` in `src/mapdraw.c` clips it with `msClipPolygonRect` in
`src/mapprimitive.cpp`, to the extent grown by the class's clip buffer: per style, the largest of
its symbol's default size, its `SIZE`, and its `WIDTH`, grown by its larger `OFFSET` unless that is
a line offset, or its `MINSIZE` or `MINWIDTH` where larger, plus 1, rounded half to even; the
largest of the styles, plus 2. It is clipped in map units, or, where a style of the class has a
`GEOMTRANSFORM`, in pixels after the transformation, to the image so grown. Each line runs across
the clipped polygon's bounding box grown by 0.7072 widths, so that the box sets where its dashes
fall, is dashed by `PATTERN` from its first point, the lengths scaled by the factor the bounds
applied to the width, and is stroked with butt caps. The stroke is cut from the polygon as
MapServer's Clipper cuts it ([ADR-0032](../adr/0032-hatch-cut-by-i-overlay.md)): the intersection of
the polygon with the stroke's rectangles, both read under the even-odd rule, every coordinate
rounded to a hundredth of a pixel; so where stroked lines overlap, at a `SIZE` below the `WIDTH`,
and where the polygon's rings overlap, the hatch is empty. A `WIDTH` of 0 makes MapServer divide the
pattern by 0, and AGG draws its infinite dashes as solid lines. `OFFSET` moves the polygon after the
clip. The cut is filled under the non-zero rule through the AGG output format's gamma
([`style-symbol.md`](style-symbol.md)).

A hatch that would not end or that costs past its bounds is not drawn, and the layer warns with
`hatch-not-drawn` (Q-9): a spacing not above 0; a pattern with a length below 0 or not finite,
or whose lengths add up to 0; a spacing that no longer moves the lines where they cross the
image, so far from the map's origin; more than 1 048 576 lines, 2^23 pixels of lines, or 2^20
dashes and gaps over the image; a polygon of more than 16 384 vertices; or more than 2^21
crossings for the cut to find: those of the polygon's edges with the stroke, estimated from the
bands of the lines each edge reaches across, and of the stroke with itself where lines lie closer
than their width, and sixteen for each crossing of the polygon with itself, counted on a grid
that gives up past the bound. MapServer loops forever on the first three and for as long as it
takes on the rest. Only the lines that cross the image are laid; the others draw nothing.

**Not yet as MapServer draws a hatch.** A size, width, or angle bound to a property or given by
an expression is drawn as if unset, and counts so in the clip buffer too, where MapServer counts
the values bound for the feature drawn before, since it computes the buffer before it binds the
feature's. A pixmap or SVG symbol of the class counts in the clip buffer at a default size of 1,
where MapServer takes its image's height. i_overlay snaps points that come within a hundredth of
a pixel of an edge, which Clipper does not. The polygon of any other style is not clipped, which
draws the same.

**Not yet as MapServer draws it at a point.** A point layer's style without a symbol it draws —
none, 0, a TrueType, SVG, hatch, or simple symbol, or a name no symbol carries — draws a square of
its size, where MapServer draws nothing for none and 0 and the symbol or image otherwise. A style a
`GEOMTRANSFORM` places ([`style-symbol.md`](style-symbol.md)) draws nothing for none and 0, as
MapServer does, its pixmap as at a point, and the square for the others, at a scale factor of 1. A
symbol, a line, a polygon's offset and outline, and a hatch are scaled by `RESOLUTION` over
`DEFRESOLUTION`; `SIZEUNITS` other than `pixels` and `SYMBOLSCALEDENOM` do not scale them yet
([`scale.md`](scale.md)).
