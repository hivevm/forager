# `SYMBOL` (style) and `GEOMTRANSFORM`

**Where:** `STYLE`. **IR:** the style's `symbol` and `geom_transform`.

**Meaning.** `SYMBOL` names the symbol the style draws: by its name, by MapServer's index into the
[symbols](symbol.md) (at least 0, a fraction cut as `loadStyle` casts it; 0 is the default symbol),
or bound to a property. A name resolves at import to the index of the first symbol of that name,
without regard to case, as `msGetSymbolIndex` in `src/mapfile.c` resolves it. MapServer reads a
name it does not find as an image file and fails the load where there is none; the frontend, which
reads no image at load, keeps the name and warns. An index past the last symbol is a warning too:
MapServer draws nothing for it.

`GEOMTRANSFORM` draws the style on another geometry than the feature's: `start`, `end`,
`vertices` (every point of a line but its ends), `centroid`, `labelpnt` (the IR's `labelpoint`),
`labelpoly`, and `labelcenter` are carried over, written quoted or bare.
`msStyleSetGeomTransform` in `src/mapgeomtransform.c` takes the first of its names, in the order
`start`, `end`, `vertices`, `bbox`, `labelpnt`, `labelpoly`, `labelcenter`, `centroid`, that the
value begins with, without regard to case, so `'startpoint'` is `start`; a value that begins with
none is no transform, and the frontend warns. A label's style draws at its `labelpnt` unless it
names another ([`label.md`](label.md)). MapServer's other transform, `bbox`, and its expressions
are not yet carried over, and are a warning; the style draws the feature's own geometry.

**Drawing a transform.** On a line or a polygon layer, `msDrawTransformedShape` in
`src/mapgeomtransform.c` draws the style's symbol, as at a point, on the feature's unclipped
shape in pixels, its vertices simplified as `msTransformShapeSimplify` in `src/mapprimitive.cpp`
simplifies them (a vertex less than a pixel from the one kept before it is dropped, and a
polygon's first two and last two are kept): `start` at the first point of each line or ring,
`end` at the last, `vertices` at every other, each only inside the image, and `centroid` once, at
the centre of the shape's segments weighted by their lengths (`msGetPolygonCentroid`), wherever it
falls. A collection's members are one shape. With `ANGLE AUTO` the symbol turns with the segment
at `start` and `end`, and at a vertex halfway between its two segments (`calcOrientation` and
`calcMidAngle`). `labelpnt` and `labelpoly` draw nothing there, nor does `labelcenter`, on which
`msDrawTransformedShape` fails, nor does a style without a symbol or with symbol 0, as
`msDrawMarkerSymbol` draws none. A line layer that draws its features style
by style draws the transformed styles with its first style, before the rest, and its first style,
when it has an outline, as its outline prepares it. A point layer draws every style at its points,
whatever its transform. The symbols a transform places count towards a layer's bound on the
points of its markers ([`line.md`](line.md), Q-9).

A part the simplification empties draws nothing in Forager: a line of fewer than two points, or
one simplified to fewer, and a ring of fewer than four. MapServer keeps such a part with no points
and still reads its first point for `start` — in pixels where the simplification began on it, in
map units where it skipped a part too short to begin — and for `end` the point before its first,
outside the part.

**A polygon's fill.** A polygon layer's style with a `COLOR` and without a symbol, or with a
simple one, fills the polygon under the even-odd rule, as `msDrawShadeSymbol` in
`src/maprendering.c` and `agg2RenderPolygon` in `src/mapagg.cpp` fill it, and a hatch symbol
hatches it ([`symbol.md`](symbol.md)). Both go through the gamma of MapServer's AGG output format:
a pixel covered by at least 0.75 of the polygon is filled fully, one covered less in proportion,
so that edges come out darker than their coverage; a symbol's fill and a line do not. The gamma
is MapServer's default of 0.75: `FORMATOPTION "GAMMA=…"` of the output format and `PROCESSING
"GAMMA=…"` of a layer are not carried yet.

**Repeated directives.** A later directive replaces an earlier one, but for a binding:
`loadStyle` never frees a property's binding when a later directive gives it a number, and
`bindStyle` in `src/maputil.c` draws the binding, so `WIDTH [w] WIDTH 3` is bound to `w`. Of
`SIZE`, a later directive frees only an expression, which `bindStyle` evaluates after the binding:
`SIZE [a] SIZE ([b] + 1)` is the expression, `SIZE [a] SIZE ([b] + 1) SIZE 5` the binding `a`.
`OFFSET` and `POLAROFFSET` keep the binding of each of their two numbers so, and a later `OFFSET`
replaces a line offset of −99 or −999 ([`offset.md`](offset.md)).
