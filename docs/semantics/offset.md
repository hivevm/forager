# `OFFSET` and `POLAROFFSET`

**Where:** `STYLE`. **IR:** the style's `offset` `[x, y]`, `line_offset`, and `polar_offset`
`[distance, angle]`; each number may be a [binding](binding.md).

**Meaning.** `OFFSET x y` shifts what the style draws by `x` and `y` pixels, `y` downwards.
MapServer gives `y` two values a meaning of their own (`MS_STYLE_SINGLE_SIDED_OFFSET` and
`MS_STYLE_DOUBLE_SIDED_OFFSET` in `src/mapserver.h`, used by `msDrawLineSymbol` in
`src/maprendering.c`): with `y` of −99 a line is drawn parallel to the feature's at the distance
`x`, with −999 on both sides of it. The frontend writes these as the `line_offset` they mean, its
`distance` the `x`, and keeps `offset` for a true shift. `POLAROFFSET d a` shifts by `d` pixels
in the direction `a` degrees, counter-clockwise from rightwards on the image, and moves only a
symbol drawn at a point: `msAdjustMarkerPos` in `src/maprendering.c` moves the point by the polar
offset, where its distance or its angle is not 0, then by the `OFFSET`, whose `y` of −99 or −999
counts as a number there, before it places the symbol's anchor point. All of them scale as
[`scale.md`](scale.md) states.

**The offset curve.** `msOffsetPolyline` in `src/maputil.c` draws `-99` and `-999` with
`msOffsetCurve`. That function first asks GEOS (`msGEOSOffsetCurve` in `src/mapgeos.c`:
`GEOSOffsetCurve_r` with 4 segments a quadrant, mitre joins, and a mitre limit of 1.5 times the
distance), and only where GEOS fails, or MapServer is built without it, computes the curve itself.
MapServer's releases and its reference images are built with GEOS. Forager draws MapServer's own
curve: every vertex moves along the normals of the segments it joins, to their mitre point, or to
the average of the two normals where the line turns by less than about 17.5° or more than about
162.5°. A segment of length 0 has no normal. The two curves differ where a turn is sharp, since
GEOS bounds a mitre and the own curve does not, and where a segment is shorter than the distance on
the inside of a turn, since GEOS removes the loop such a segment makes and the own curve keeps it.
A positive distance lies below a line drawn rightwards in the image.

MapServer clips a line to the image, grown by the widest symbol of its class, before it moves it.
Forager moves the line first and clips it in the renderer, so a line that leaves the image is
offset as one line where MapServer offsets each clipped part.
