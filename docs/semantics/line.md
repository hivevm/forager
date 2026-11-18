# `PATTERN`, `GAP`, `INITIALGAP`, `LINECAP`, `LINEJOIN`, `LINEJOINMAXSIZE`, `ANTIALIAS`

**Where:** `STYLE`. **IR:** the style's `pattern`, `gap`, `initial_gap`, `line_cap`,
`line_join`, `line_join_max_size`, `aliased`.

**Meaning**, as `loadStyle` in `src/mapfile.c` reads them and `msDrawLineSymbol` in
`src/maprendering.c` draws them:

- `PATTERN … END`: 2 to 10 lengths, dash and gap alternating, as written; drawn scaled with the
  line's width as [`scale.md`](scale.md) states, so dashes grow with a line that grows.
- `GAP`: the distance between symbols drawn along a line, any number. A positive one keeps the
  symbols upright; a negative one turns them with the line, its size the distance
  (`msImagePolylineMarkers` with `auto_angle` 1). 0 draws the symbol as a brush along the line: a
  vector, ellipse, pixmap, or TrueType symbol brushes the line with its brush tile, as
  [`symbol.md`](symbol.md) states, and any other is drawn as a simple symbol's line. Each
  line of a shape counts from its start, a segment carrying on what the one before left over; a line
  on which no symbol falls, without `INITIALGAP`, and longer than the symbol is wide takes one at
  half its length. A pixmap symbol is drawn there as at a point ([`symbol.md`](symbol.md)), without
  a `COLOR`, turned or not as the gap says; its marker is the image's width and height times its
  scale, at least 1 each way, not the size `msGetMarkerSize` cuts to whole pixels, and it sizes the
  anchor point's shift, the length a line must pass to take its one marker, and how far outside the
  image a marker is still drawn: a 32 × 32 image at `SIZE 15.5` and `ANCHORPOINT 0 0` moves 7.75
  pixels each way along a line, 7.5 at a point. A later style of a cached line layer whose symbol is
  a pixmap and whose `GAP` is not 0 is drawn without a `COLOR`, as `msDrawVectorLayer` draws it
  (`src/mapdraw.c`). A pixmap at a scale of 0, as a scale factor of 0 gives, draws nothing along a
  line, as at a point; MapServer scales its gap as the size to 0 too and places its markers 0 apart
  without end (`computeSymbolStyle`, `msImagePolylineMarkers`). Forager draws the markers of one
  layer, those along its lines with those its `GEOMTRANSFORM`s place and, in a point layer, those at
  its features, up to a million points of their outlines and fills in all, a pixmap's image counting
  one and any symbol at least one; the square a point layer draws for no symbol or symbol 0, which
  MapServer does not draw ([`symbol.md`](symbol.md)), spends none. The markers of a feature, of its
  line, or of a transformed shape, in a style that would pass that are not drawn, nor any of the
  layer's after them, with a warning (Q-9).
- `INITIALGAP`: the distance before the first dash or symbol, at least 0; without it a pattern
  starts at 0, and symbols at half the gap (`msImagePolylineMarkers`).
- `LINECAP`: `butt`, `round` (the default), `square`, `triangle`. `LINEJOIN`: `none` (the
  default), `round`, `miter`, `bevel`. `LINEJOINMAXSIZE`: how far a miter reaches as a multiple
  of the width, above 0, default 3.
- `ANTIALIAS`: `true` or `false`, or a word of the same token value, such as `on` or `off`. A
  `false` strokes the style's line aliased: every pixel the stroke's outline covers at all takes
  the full colour, as `agg2RenderLine` in `src/mapagg.cpp` renders it through AGG's
  `renderer_scanline_bin_solid`; the IR holds `aliased: true`. A `true` changes nothing, so a
  `false` before it stays, as `loadStyle` reads it. Only a line `msDrawLineSymbol` strokes, in
  `src/maprendering.c`, takes the flag: a line, and a polygon's outline in a style without a
  `COLOR`. The outline `msDrawShadeSymbol` draws about a polygon's fill, a symbol's outline, and a
  hatch stay anti-aliased. A label's `ANTIALIAS` is read and ignored
  ([`label.md`](label.md)).

**Vertices.** A line or a polygon, outlined, stroked, or hatched, reaches pixels as
`msTransformShapeSimplify` in `src/mapprimitive.cpp` puts it there, the transform mode
`msImageStartLayer` in `src/mapdraw.c` keeps from the AGG renderer for a layer without
`PROCESSING "APPROXIMATION_SCALE=…"`, which the style IR does not carry: each vertex by the
inverse of the cellsize, and a vertex not more than a pixel from the one kept before it dropped. A
line keeps its first vertex and its last unless that equals the one kept before; a polygon's ring,
in a line layer too, its first two and its last two. A line left with fewer than two vertices and
a ring of fewer than four draw nothing.

**Clip.** `msDrawShape` in `src/mapdraw.c` clips a line shape that reaches beyond the map's extent
before `msDrawLineSymbol` draws it, with `msClipPolylineRect` in `src/mapprimitive.cpp`, and so
does Forager. The rectangle is the extent grown by the class's buffer in pixels, times the
cellsize: per style, the largest of its symbol's default size, its `SIZE`, and its `WIDTH`, scaled,
or its `MINSIZE` or `MINWIDTH` where larger, plus 1, rounded; the largest of the class's styles.
Where a style of the class has a `GEOMTRANSFORM`, the shape is turned into pixels first and the
image grown by the buffer cuts it. Each segment is cut in x, then in y; a line that leaves the
rectangle ends there, and one that comes back is a line of its own from there. Its dashes, its
`INITIALGAP`, and its symbols count from where the clip begins it, and its vertices are simplified
(above) from there. A layer of `PROCESSING "POLYLINE_NO_CLIP"`, whatever its value, is not clipped
(`src/mapdraw.c:1341-1344`): the IR's `unclipped_lines`.

**Not yet as MapServer places symbols along a line.**

- A polygon's rings outlined in a line layer, which `msDrawShape` clips with
  `msClipPolygonRect` and a buffer 2 larger, are not clipped; they are simplified whole, so
  where one crosses the edge of the map the vertices kept near the edge can differ.
- The renderer clips every path near the image ([ADR-0027](../adr/0027-clip-to-the-image.md)),
  2 pixels beyond it and the stroke's reach, and dashes a line from where that clip begins it. A
  line `msDrawShape` leaves longer, one of `POLYLINE_NO_CLIP` or one whose class's buffer is
  wider than the renderer's margin, a stroke wider than 2 pixels with round or bevel joins, has
  its dashes counted from the renderer's clip, where MapServer counts them from the line's start.
- The renderer's clip drops a vertex that is no finite number in pixels, a segment whose ends
  lie farther apart than any finite number, and a crossing of its edge that is no finite number,
  and draws the line or polygon through what is left. The layer warns with `path-not-drawn`, once
  ([ADR-0049](../adr/0049-third-party-failures-guarded.md)).
- A line brushed with its tile, of a `GAP` of 0, that the renderer's clip cuts, as one of
  `POLYLINE_NO_CLIP` beyond the class's buffer and 2 pixels, draws the segment cut from an end
  rounded to the clip's edge, without the join before it, where MapServer saturates the line's
  coordinates instead. The pattern's place along the rest of the line is MapServer's.
- A piece of a brushed line is not drawn where AGG's C++ is undefined: where the pattern's place
  would pass an `int`, beside a tile some 4096 pixels high, or where a join lies so far from a
  short segment that the distances to it would pass an `int`. The layer warns with
  `pixmap-not-drawn` for a pixmap's tile and `symbol-not-drawn` for any other, once for each
  kind, naming no symbol ([ADR-0049](../adr/0049-third-party-failures-guarded.md)).
- A brushed line's segment longer than 1024 pixels close to 45°, or a join whose bisectrix falls
  on an exact half, can differ from MapServer by a pixel's shade: agg-rust divides such a
  segment, and rounds a half away from zero, otherwise than AGG 2.4.
