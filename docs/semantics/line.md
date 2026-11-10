# `PATTERN`, `GAP`, `INITIALGAP`, `LINECAP`, `LINEJOIN`, `LINEJOINMAXSIZE`

**Where:** `STYLE`. **IR:** the style's `pattern`, `gap`, `initial_gap`, `line_cap`,
`line_join`, `line_join_max_size`.

**Meaning**, as `loadStyle` in `src/mapfile.c` reads them and `msDrawLineSymbol` in
`src/maprendering.c` draws them:

- `PATTERN … END`: 2 to 10 lengths, dash and gap alternating, as written; drawn scaled with the
  line's width as [`scale.md`](scale.md) states, so dashes grow with a line that grows.
- `GAP`: the distance between symbols drawn along a line, any number. A positive one keeps the
  symbols upright; a negative one turns them with the line, its size the distance
  (`msImagePolylineMarkers` with `auto_angle` 1). 0 draws the symbol as a brush along the line. Each
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

**Not yet as MapServer places symbols along a line.**

- `msDrawShape` in `src/mapdraw.c` clips a line that leaves the map's extent to the image grown by
  a buffer of the styles' largest size or width before `msDrawLineSymbol` draws it, unless the
  layer has `PROCESSING "POLYLINE_NO_CLIP=true"`. Each clipped piece is a line of its own: its
  symbols start again at `INITIALGAP`, or at half the gap, and a piece on which none falls takes
  its own marker at half its length. Forager does not clip, and counts from the start of the
  whole line, so the symbols of a line that crosses the edge of the map can stand elsewhere.
- `msTransformShapeSimplify` in `src/mapprimitive.cpp` drops each vertex of a line less than a
  pixel from the one kept before it. Forager keeps every vertex, so on a line of dense vertices
  its segments, and the angles of the symbols a negative `GAP` turns with them, can differ.
- With a `GAP` of 0, `msDrawLineSymbol` tiles a pixmap's image along the line
  (`renderLineTiled`). Forager strokes the line instead as a simple symbol's, in the `COLOR` or
  else the `OUTLINECOLOR` for the first style of a class, and a later style without a `COLOR` not
  at all.
