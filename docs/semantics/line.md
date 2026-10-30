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
  one at half its length. Forager draws the symbols of one line up to a million points of their
  outlines and fills, and none of that line's beyond, with a warning (Q-9).
- `INITIALGAP`: the distance before the first dash or symbol, at least 0; without it a pattern
  starts at 0, and symbols at half the gap (`msImagePolylineMarkers`).
- `LINECAP`: `butt`, `round` (the default), `square`, `triangle`. `LINEJOIN`: `none` (the
  default), `round`, `miter`, `bevel`. `LINEJOINMAXSIZE`: how far a miter reaches as a multiple
  of the width, above 0, default 3.
