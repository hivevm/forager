# `PATTERN`, `GAP`, `INITIALGAP`, `LINECAP`, `LINEJOIN`, `LINEJOINMAXSIZE`

**Where:** `STYLE`. **IR:** the style's `pattern`, `gap`, `initial_gap`, `line_cap`,
`line_join`, `line_join_max_size`.

**Meaning**, as `loadStyle` in `src/mapfile.c` reads them and `msDrawLineSymbol` in
`src/maprendering.c` draws them:

- `PATTERN … END`: 2 to 10 lengths, dash and gap alternating, as written; drawn scaled with the
  line's width as [`scale.md`](scale.md) states, so dashes grow with a line that grows.
- `GAP`: the distance between symbols drawn along a line, any number. A positive one keeps the
  symbols upright; a negative one turns them with the line, its size the distance
  (`msImagePolylineMarkers` with `auto_angle` 1). 0 draws the symbol as a brush along the line.
  Each line of a shape counts from its start, a segment carrying on what the one before left over;
  a line on which no symbol falls, without `INITIALGAP`, and longer than the symbol is wide takes
  one at half its length. Forager draws the symbols along the lines of one layer up to a million
  points of their outlines and fills in all: the markers of a feature's line in a style that
  would pass that are not drawn, nor any of the layer's after them, with a warning (Q-9).
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
