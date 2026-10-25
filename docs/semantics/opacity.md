# `OPACITY` and `OUTLINEWIDTH`

**Where:** `STYLE`. **IR:** the style's `opacity`, percent, and `outline_width`, pixels; each a
number or a [binding](binding.md).

**Meaning.** `OPACITY` is how opaque the style draws, 0 to 100, default 100; `loadStyle` in
`src/mapfile.c` truncates a number to an integer and clamps it to 0…100, and the frontend stores
the result. An opacity below 100 replaces the alpha of the style's colours, a `#rrggbbaa`'s
included, with `opacity × 2.55` rounded (`loadStyle` at its `END`, and `bindStyle` in
`src/maputil.c` for a bound one); an opacity of 100 leaves the colours' alpha as written.

`OUTLINEWIDTH`, at least 0, default 0, outlines a line of a `LINE` layer as RFC 49 states:
`msOutlineRenderingPrepareStyle` in `src/maprendering.c`, called from `msDrawVectorLayer` in
`src/mapdraw.c`, widens the line by twice the `OUTLINEWIDTH`, draws it in the `OUTLINECOLOR`
first, and the line in its `COLOR` over it. The
outline of a symbol is not it: `computeSymbolStyle` draws a symbol's outline as wide as the
style's `WIDTH` ([`width.md`](width.md)).
