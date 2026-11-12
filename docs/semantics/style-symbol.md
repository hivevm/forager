# `SYMBOL` (style) and `GEOMTRANSFORM`

**Where:** `STYLE`. **IR:** the style's `symbol` and `geom_transform`.

**Meaning.** `SYMBOL` names the symbol the style draws: by its name, by MapServer's index into the
[symbols](symbol.md) (at least 0, a fraction cut as `loadStyle` casts it; 0 is the default symbol),
or bound to a property. A name resolves at import to the index of the first symbol of that name,
without regard to case, as `msGetSymbolIndex` in `src/mapfile.c` resolves it. A name no symbol
carries is an image file, as `msAddImageSymbol` in `src/mapsymbol.c` reads it once the map is read:
the frontend appends a symbol named by the file name to the map's symbols, after the symbol file's,
layer by layer and class by class, a class's styles before its labels' styles, as
`classResolveSymbolNames` takes them; an SVG symbol for a `.svg` extension and a pixmap otherwise,
its `IMAGE` the name against the main mapfile's directory inside the base context (Q-8), as
MapServer's `mappath`. A later style of the same name, in any case, refers to the same symbol. No
file is read at load: one that does not exist warns with `pixmap-not-drawn` as the symbol is drawn
([`symbol.md`](symbol.md)): at a point, along a line with a `GAP`, or over a polygon, where its
image does not load, and along a line without a `GAP`, where MapServer tiles the image and Forager
draws a line in its place without reading it. A path that leaves the base context is a load error,
as `IMAGE`'s. A name that begins with `http`, without regard to case, MapServer fetches
with curl, and fails the load where it cannot; Forager never reads it, and the frontend warns. The
name still takes its index, as `msAddImageSymbol` grows the symbols before it fetches
(`src/mapsymbol.c`, 402–406), so that the images after it are numbered as MapServer numbers them: a
pixmap symbol of that name without an image, a `.svg` name among them, drawn as a pixmap whose
image does not load. An empty name is a load error, as MapServer's: `msAddImageSymbol` makes no
symbol of it (398–399), and
`classResolveSymbolNames` fails with "Undefined symbol" (`src/mapfile.c`, 3723–3762). An index past
the last symbol, those the styles append among them, is a warning too: MapServer draws nothing for
it.

In a style document, a `symbol` that is a name resolves as `msGetSymbolIndex` resolves it
(`src/mapfile.c`, 408–426): to the first symbol of that name from index 1, without regard to case. A
name no symbol carries draws nothing, as a document has no image file to read it as, and the layer
warns with `symbol-unknown` once for each such name of its classes' styles.

`GEOMTRANSFORM` draws the style on another geometry than the feature's: `start`, `end`,
`vertices` (every point of a line but its ends), `bbox`, `centroid`, `labelpnt` (the IR's
`labelpoint`), `labelpoly`, and `labelcenter` are carried over, written quoted or bare.
`msStyleSetGeomTransform` in `src/mapgeomtransform.c` takes the first of its names, in the order
`start`, `end`, `vertices`, `bbox`, `labelpnt`, `labelpoly`, `labelcenter`, `centroid`, that the
value begins with, without regard to case, so `'startpoint'` is `start`; a value that begins with
none is no transform. MapServer's expressions are not yet carried over. A class's style of either
draws the feature's own geometry, and the frontend warns with `unsupported-value`. A label's style
draws at its `labelpnt` unless it names another. A label with a style of any other transform, of
a name that is no transform, or of an expression is left out, and its class's other labels with
it, with the one warning `label-not-drawn` ([`label.md`](label.md)).

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
`msDrawTransformedShape` fails the layer's draw, so the frontend warns of it on a class's style
of a line or polygon layer; nor does a style without a symbol or with symbol 0, as
`msDrawMarkerSymbol` draws none. `bbox` draws no symbol at a point but a polygon, the bounds of the
shape in pixels as a ring of five points from their lower left corner up and around: each side
is held within the image grown by a padding, the larger of the style's `WIDTH` (1 by default)
and `SIZE` (-1 by default) and 3 pixels, cut to an `int`, so the left and the upper side at no
less than minus the padding and the right and the lower at no more than the image's width or
height and the padding. `msDrawShadeSymbol` draws that ring as a polygon of a polygon layer is
drawn, below, symbol 0 included: without a `COLOR`, stroked in the `OUTLINECOLOR`, and with
neither, not at all. A `WIDTH` or `SIZE` past an `int` gives the padding `INT_MIN`, as the x86-64
build at the pin converts it; the lower bounds then hold nothing and the upper ones fall far below
the image. A line layer that draws its features style
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
`src/maprendering.c` and `agg2RenderPolygon` in `src/mapagg.cpp` fill it, a hatch symbol hatches
it, and a pixmap symbol fills it with its image tiled, with or without a `COLOR`
([`symbol.md`](symbol.md)). The fill and the hatch go through the gamma of MapServer's AGG output
format: a pixel covered by at least 0.75 of the polygon is filled fully, one covered less in
proportion, so that edges come out darker than their coverage; a symbol's fill, a tile's, and a line
do not. The gamma is MapServer's default of 0.75: `FORMATOPTION "GAMMA=…"` of the output format and
`PROCESSING "GAMMA=…"` of a layer are not carried yet.

**Repeated directives.** A later directive replaces an earlier one, but for a binding:
`loadStyle` never frees a property's binding when a later directive gives it a number, and
`bindStyle` in `src/maputil.c` draws the binding, so `WIDTH [w] WIDTH 3` is bound to `w`. Of
`SIZE`, a later directive frees only an expression, which `bindStyle` evaluates after the binding:
`SIZE [a] SIZE ([b] + 1)` is the expression, `SIZE [a] SIZE ([b] + 1) SIZE 5` the binding `a`.
`OFFSET` and `POLAROFFSET` keep the binding of each of their two numbers so, and a later `OFFSET`
replaces a line offset of −99 or −999 ([`offset.md`](offset.md)).
