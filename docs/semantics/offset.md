# `OFFSET` and `POLAROFFSET`

**Where:** `STYLE`. **IR:** the style's `offset` `[x, y]`, `line_offset`, and `polar_offset`
`[distance, angle]`; each number may be a [binding](binding.md).

**Meaning.** `OFFSET x y` shifts what the style draws by `x` and `y` pixels, `y` downwards.
MapServer gives `y` two values a meaning of their own (`MS_STYLE_SINGLE_SIDED_OFFSET` and
`MS_STYLE_DOUBLE_SIDED_OFFSET` in `src/mapserver.h`, used by `msDrawLineSymbol` in
`src/maprendering.c`): with `y` of −99 a line is drawn parallel to the feature's at the distance
`x`, with −999 on both sides of it. The frontend writes these as the `line_offset` they mean, its
`distance` the `x`, and keeps `offset` for a true shift. `POLAROFFSET d a` shifts by `d` pixels
in the direction `a` degrees. All of them scale as [`scale.md`](scale.md) states.
