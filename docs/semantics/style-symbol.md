# `SYMBOL` (style) and `GEOMTRANSFORM`

**Where:** `STYLE`. **IR:** the style's `symbol` and `geom_transform`.

**Meaning.** `SYMBOL` names the symbol the style draws: by its name, by MapServer's index into the
[symbols](symbol.md) (at least 0, a fraction cut as `loadStyle` casts it to an `int`, and a number
past an `int` `INT_MIN`, which draws nothing as an index past the symbols does, though
`msDrawShape` fails the draw of a shape it clips on either, `src/mapdraw.c` 2470–2474, which
Forager does not port; 0 is the default symbol), or bound to a property.
A name resolves at import to the index of the first symbol of that name,
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
none is no transform; a class's style of it draws the feature's own geometry, a label's style
draws at its `labelpnt`, and the frontend warns with `unsupported-value`. A style's expression,
`(buffer([shape], 5))` and the like, is carried as its transform tree (below). Of a line or a
polygon layer, the engine draws a class's style of one as `msDrawTransformedShape` draws it, on the
shape in pixels (*A class style's expression*, below); of a tree `not_drawn`, it draws nothing of
the style and warns `geomtransform-not-drawn`, naming the reason, the class's other styles drawn in
their passes as before.
A point layer draws the style at its points without a warning, as `pointLayerDrawShape` draws
every style whatever its transform. A label's style draws at its `labelpnt` unless it names
another. A label with a style of any other transform or of an expression is left out, and its
class's other labels with it, with the one warning `label-not-drawn` ([`label.md`](label.md)).

**Drawing a transform.** On a line or a polygon layer, `msDrawTransformedShape` in
`src/mapgeomtransform.c` draws the style's symbol, as at a point, on the feature's unclipped shape
in pixels, its vertices simplified as `msTransformShapeSimplify` in `src/mapprimitive.cpp`
simplifies them (a vertex less than a pixel from the one kept before it is dropped, and a polygon's
first two and last two are kept): `start` at the first point of each line or ring, `end` at the
last, `vertices` at every other, each only inside the image, and `centroid` once, at the centre of
the shape's segments weighted by their lengths (`msGetPolygonCentroid`), wherever it falls. A
collection's members are one shape. With `ANGLE AUTO` the symbol turns with the segment at `start`
and `end`, and at a vertex halfway between its two segments (`calcOrientation` and `calcMidAngle`).
`labelpnt` and `labelpoly` draw nothing there, nor does `labelcenter`, on which
`msDrawTransformedShape` fails the layer's draw, so the engine warns of it on a class's style of a
line or polygon layer with `geomtransform-not-drawn`, of a style document as of a mapfile, as it
renders a layer it draws, so that `forager read` alone does not report it; nor does
a style without a symbol or with symbol 0, as `msDrawMarkerSymbol` draws none. `bbox` draws no
symbol at a point but a polygon, the bounds of the shape in pixels as a ring of five points from
their lower left corner up and around: each side is held within the image grown by a padding, the
larger of the style's `WIDTH` (1 by default) and `SIZE` (-1 by default) and 3 pixels, cut to an
`int`, so the left and the upper side at no less than minus the padding and the right and the lower
at no more than the image's width or height and the padding. `msDrawShadeSymbol` draws that ring as
a polygon of a polygon layer is drawn, below, symbol 0 included: without a `COLOR`, stroked in the
`OUTLINECOLOR`, and with neither, not at all. A `WIDTH` or `SIZE` past an `int` gives the padding
`INT_MIN`, as the x86-64 build at the pin converts it; the lower bounds then hold nothing and the
upper ones fall far below the image. A line layer that draws its features style by style draws the
transformed styles with its first style, before the rest, and its first style, when it has an
outline, as its outline prepares it. A point layer draws every style at its points, whatever its
transform. The symbols a transform places count towards a layer's bound on the points of its markers
([`line.md`](line.md), Q-9).

A part the simplification empties draws nothing in Forager: a line of fewer than two points, or
one simplified to fewer, and a ring of fewer than four. MapServer keeps such a part with no points
and still reads its first point for `start` — in pixels where the simplification began on it, in
map units where it skipped a part too short to begin — and for `end` the point before its first,
outside the part.

**A `GEOMTRANSFORM` expression.** An expression of a layer or of a style is read as MapServer's
lexer and grammar read it (`src/maplexer.l`, `src/mapparser.y`, the rules `shape_exp`, `math_exp`,
and `string_exp`), function names without regard to case, without the parentheses `loadLayer` and
`loadStyle` strip. The frontend translates it into a transform tree (ADR-0043): `[shape]`, and
`generalize`, `densify`, `simplify`, `outer`, `inner`, and `smoothsia` of a shape, nested. A number
is written, `[map_cellsize]`, or computed of those by `+ - * / % ^` and parentheses, as the grammar
reduces them; the frontend folds every operation without `[map_cellsize]` to one number. A unary `-`
leaves its operand as it is, as the rule `'-' math_exp %prec NEG { $$ = $2; }` does, while `-3`
written together is one negative number, so `[map_cellsize]-2` does not parse. `%` takes its
operands cut to `int`s, `INT_MIN` past an `int` (ADR-0017). `[data_cellsize]` is 0, as
`msGeomTransformShape` binds it on every layer but a contour layer
(`src/mapgeomtransform.c:285-291`), which Forager does not carry. `smoothsia`'s size and iterations
are 3 and 1 where they are not written, as the grammar calls `msSmoothShapeSIA(s, 3, 1, NULL)`; its
preprocessing `all` or `angle` is read without regard to case, as `strcasecmp` reads it, and any
other text is none.

An expression the grammar parses that holds any other construct is the tree `not_drawn`, naming the
first such construct in the expression's reading order: a function the tree lacks, `buffer`,
`centerline`, `difference`, `simplifypt`, `javascript`, `length`, `area`, or `round`; `binding`, an
attribute where a number or a text is due; `text`, a computed preprocessing of `smoothsia`;
`fromtext`, a shape read from a text, which the tree cannot hold; or `literal`, an operation of
literals that folds to no finite number, which a style document cannot carry: a `%` by 0 or of
`INT_MIN` by -1, on which the x86-64 build traps, a result past a double, and a literal past one,
which `atof` reads as infinite. An expression the grammar does not parse, anywhere in it, is
`not_drawn: syntax`, whatever construct comes before the error, as MapServer refuses the whole
expression; a division of literals by 0, which `math_exp` fails with `yyerror(p, "Division by
zero.")`, is one. The frontend warns `invalid-expression` at the place it stops, but on a label's
style, whose label it leaves out. A style document of a newer minor version whose tree names a
reason this reader does not know is read as not drawn, with a warning naming the reason and its
path; the tree holds it as `syntax`, the reason the engine's warning then names. A tree that nests
more than 24 levels, each node and each operator of a chain one level, fails the load with
`expression-too-deep` (Q-9), from a mapfile and from a style document alike.

**A layer's `GEOMTRANSFORM`.** A layer's `GEOMTRANSFORM` is an expression alone (RFC 89, read by
`getSymbol(1, MS_EXPRESSION)`); anything else fails the load. `msLayerNextShape` in
`src/maplayer.c` replaces each shape it reads by the expression, after the layer's filter passes
it and before it is classed or drawn (`msGeomTransformShape`, `src/mapgeomtransform.c`). The
engine draws its tree's functions:

- `generalize(s, t)` (`msGeneralize`, `src/maputil.c`): the first part of `s` alone, of its points
  the first, each at least `t` from the last kept, and the last. A polygon's other rings and a
  line's other parts are dropped.
- `densify(s, t)` (`msDensify`, `src/mapprimitive.cpp`): each segment of a line or a polygon
  longer than `t` cut into `floor(length / t) + 1` equal ones. It fails a point and a `t` not
  above 0.
- `simplify(s, t)` (`msGEOSSimplify`, `src/mapgeos.c`): `s` as GEOS's Douglas-Peucker simplifies
  it at the tolerance `t`, by a Douglas-Peucker ported from JTS (ADR-0041).
  A point shape keeps its points. Each part of a line keeps its end points and the points the
  Douglas-Peucker keeps, which splits a section at the first of its farthest points, as GEOS
  does, and needs no recursion, so no line overflows the stack. A part without points is dropped. A
  polygon's rings are grouped as `msGEOSShape2Geometry` groups them: each outer ring, by
  `msIsOuterRing`, with the inner rings whose first or second point lies in it, one polygon where
  one ring is outer, a multipolygon otherwise; a ring neither outer nor in an outer one is dropped.
  Each ring is simplified as a line, then loses its first point where it lies within `t` of the
  segment between its neighbours, and is removed below 4 coordinates. A polygon whose shell is
  removed is removed with its holes, and a shape without polygons draws nothing. The result is
  kept where geo's `Validation` finds it valid, a multipolygon as a whole, and is otherwise
  replaced by geo's `unary_union` of its polygons, where GEOS takes a buffer of 0: the two agree
  on most results, within 10⁻⁶ in the shape's unit and in another order of the vertices, but
  where the simplified ring crosses itself `unary_union` may keep another lobe than GEOS, as it
  did for 7 of 40 repaired random polygons; no case in scope simplifies a polygon. A tolerance of
  0 drops the points that lie on the segment of their section, as GEOS does; a tolerance that is
  no number keeps every point, as GEOS's comparisons with it are false. A multipolygon's
  member whose shell is removed while one of its holes is kept leaves the hole as a stray line
  in GEOS's result, which turns the shape into a line; Forager removes the member with its holes.
- `outer(s)` and `inner(s)` (`msRings2Shape`, `msIsOuterRing`, `src/mapprimitive.cpp`): the rings
  of a polygon that are outer, or inner, in their order. A ring alone is outer; another is outer
  where its first point lies in an even number of the other rings by `msPointInPolygon`. Where its
  first and second points disagree, its third decides. They fail any shape that is no polygon.
- `smoothsia(s, size, iterations[, preprocessing])` (`msSmoothShapeSIA`, `src/mapsmoothing.c`).
  Each part of three points or more is smoothed by a window of `size` points, at least 3, odd,
  and at most one below the part's count, weighed by halves away from its centre and wrapped
  around a closed ring. A point whose window runs off a line is kept. The passes are as many as
  `iterations` less one where it is above 1: the C smooths the shape it was given on both its
  first and its second pass. The preprocessing `all` adds the midpoints towards both neighbours of
  each point whose window of five is whole; `angle` adds them only where that window's path is
  over 1.3 times the distance between its ends. A size or a number of iterations past an `int` is
  `INT_MIN`, which `msSmoothShapeSIA` raises to its least.

`[map_cellsize]` is the map's cellsize times the inches of the map's `UNITS` over those of the
layer's (`src/mapgeomtransform.c:283-284`). The frontend does not carry a layer's `UNITS` and
warns of one, so the layer's unit is `initLayer`'s, meters.

A layer whose tree is `not_drawn` draws nothing, with the warning `geomtransform-not-drawn` naming
the reason: MapServer evaluates those functions through GEOS or a JavaScript engine, and fails the
others. So does a layer whose transform fails on a shape as MapServer's fails: `densify` of a point
or by a tolerance not above 0, `outer` or `inner` of a line, a division by 0, or a `%` by 0, of a
computed number. `simplify` fails, with "Executing simplify failed", where GEOS is given no geometry
or fails: a tolerance below 0, which GEOS's simplifier refuses; a point shape without points, a line
whose first part has fewer than 2 points, or a polygon whose first ring has fewer than 4, which
`msGEOSShape2Geometry` does not convert; and a line's part of 1 point, or a ring of a polygon of
fewer than 4 points or not closed, which GEOS cannot build; a ring in no polygon is never built and
fails nothing. The engine treats a geometry collection, or a multipoint, whose parts the IR does not
keep, the same way. MapServer fails `msGeomTransformShape` on these: `msDrawVectorLayer` returns a
failure (`src/mapdraw.c`, 1427–1434), and `msDrawMap` frees the image and returns none (508–519), so
MapServer makes no map at all. Forager deviates: the layer draws nothing, with the warning, and the
rest of the map is drawn. `msDrawVectorLayer` reads no shape past the first whose class it finds
beyond the layer's `MAXFEATURES` (`src/mapdraw.c:1282-1286`), so a shape past it, or one the filter
leaves out, is never transformed and fails nothing. A layer draws nothing, with
`geomtransform-over-limit`, where its transforms add more than 1,000,000 points or visit more than
100,000,000; the smoothing passes and the tests of `outer` and `inner` count as visits, and so does
what `simplify` visits: the inner points of each section its Douglas-Peucker scans, charged as it
goes, the tests that group a polygon's rings, and the pairs of segments and of rings the validity
test and the repair compare. The points the repair returns beyond those it was given count as added.
The validity test is charged the square of a simplified ring's segments, so a polygon whose
simplified ring keeps more than about 10,000 points passes the bound alone and its layer draws
nothing, where MapServer draws it. In its worst case, as on a zigzag of falling amplitude, the
Douglas-Peucker scans about n²/2 points for a part of n points kept, so such a line of more than
about 14,000 points passes the bound too, and its layer draws nothing.

**A class style's expression.** On a line or a polygon layer, `lineLayerDrawShape` and
`polygonLayerDrawShape` (`src/mapdraw.c:2140-2148`, `2304-2311`) hand a class style's expression,
in its scale range, the feature's unclipped shape in pixels, as `msDrawShape` transforms it before
it clips (`:2505-2518`): its lines and rings simplified as for the keyword transforms above, a part
the simplification leaves without points kept empty, as `msTransformShapeSimplify` keeps it, and a
multipolygon's rings listed flat. A shape left without a point draws no style at all, as
`msDrawShape` returns before it draws one (`:2506-2507`, `2577-2580`), so its tree is not evaluated.
`msDrawTransformedShape` (`src/mapgeomtransform.c:198-240`) evaluates the tree on that shape, so
its numbers are pixels:
`generalize([shape], 50)` keeps, of the first part alone, its first point, each point at least 50
pixels from the last kept, and its last. A line result is stroked as `msDrawLineSymbol` strokes a
line of a line layer, a part of fewer than two points drawing nothing; a point or a polygon result
is shaded as `msDrawShadeSymbol` shades a polygon, below. Each is drawn in the style, as its pass
prepares it: a line layer draws such a style in the pass of a style of a keyword transform, with
its first style, before the rest. The functions are those of a layer's transform, above, with the
same failures.

`[map_cellsize]` of a style's tree is the layer's value, above. MapServer sets none for a style:
`msDrawTransformedShape` leaves its parser's `dblval` unset, so a style's `[map_cellsize]` reads an
uninitialised double at the pin (`src/mapparser.y:1121-1128`). No case of msautotest uses it;
Forager deviates to the one value the name has on a layer
(ADR-0043).

Where a style's tree fails on a shape, MapServer's draw of the layer fails and `msDrawMap` makes no
map (`src/mapdraw.c:1402-1405`, `508-519`). Forager deviates: the style draws nothing on that
shape, its class's other styles and the rest of the map are drawn, and the engine warns
`geomtransform-not-drawn` once for each such style and render, naming the style and its first
failure in the order the shapes are drawn. A geometry collection fails as on a layer.

The trees of a layer's class styles share the bounds of a layer's transform (Q-9) with the layer's
`GEOMTRANSFORM`, in one budget a layer and render: the points they add, 1,000,000, and the points
they visit, 100,000,000. Where an evaluation passes a bound, the layer's style trees draw nothing on
that shape and on every later shape of the layer; the shapes drawn before stay drawn, so what is
drawn depends on the order of the features. The engine warns `geomtransform-over-limit` once for the
layer and render, whatever the number of style trees the bound stops.

**A polygon's fill.** A polygon layer's style with a `COLOR` and without a symbol, or with a
simple one, fills the polygon under the even-odd rule, as `msDrawShadeSymbol` in
`src/maprendering.c` and `agg2RenderPolygon` in `src/mapagg.cpp` fill it, a hatch symbol hatches
it, a pixmap symbol fills it with its image tiled, with or without a `COLOR`, and a vector symbol
with the symbol tiled ([`symbol.md`](symbol.md)). The fill and the hatch go through the gamma of
MapServer's AGG output format: a pixel covered by at least 0.75 of the polygon is filled fully,
one covered less in proportion, so that edges come out darker than their coverage; a symbol's
fill, a tile's, and a line do not. The gamma is MapServer's default of 0.75:
`FORMATOPTION "GAMMA=…"` of the output format and `PROCESSING "GAMMA=…"` of a layer are not
carried yet.

**Repeated directives.** A later directive replaces an earlier one, but for a binding:
`loadStyle` never frees a property's binding when a later directive gives it a number, and
`bindStyle` in `src/maputil.c` draws the binding, so `WIDTH [w] WIDTH 3` is bound to `w`. Of
`SIZE`, a later directive frees only an expression, which `bindStyle` evaluates after the binding:
`SIZE [a] SIZE ([b] + 1)` is the expression, `SIZE [a] SIZE ([b] + 1) SIZE 5` the binding `a`.
`OFFSET` and `POLAROFFSET` keep the binding of each of their two numbers so, and a later `OFFSET`
replaces a line offset of −99 or −999 ([`offset.md`](offset.md)).
