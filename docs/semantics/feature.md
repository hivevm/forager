# `FEATURE` and `PROCESSING "ITEMS=…"`

**Where:** `LAYER`. **IR:** the layer's `features`, each with its `type` (`point`, `line`, or
`polygon`), its `parts`, each a list of `[x, y]`, its `properties`, each a `name` and a string
`value`, and its `text` ([ADR-0033](../adr/0033-inline-features-in-the-style-ir.md)).

**Meaning.** A `FEATURE` block writes one feature into the layer. `loadLayer` in `src/mapfile.c`
refuses one before the layer's `TYPE`, and gives its shape the type of the `TYPE` read so far:
polygon for `POLYGON`, line for `LINE`, point for every other, `CIRCLE` included; a later `TYPE`
leaves it as it was read. `loadFeature` reads the block's statements in order:

- `POINTS … END` adds one part, an empty one included (`msAddLine`): pairs of numbers, `x y`, as
  `loadFeaturePoints` reads them; anything else in it, or a number without its pair, fails the
  load.
- `ITEMS "…"` replaces the feature's values with the string split at `;`, an empty value kept
  between two separators and after the last, and none of an empty string
  (`msStringSplitComplex` with `MS_ALLOWEMPTYTOKENS`).
- `TEXT "…"` replaces the feature's text. The labels do not read it yet.
- `WKT "…"` replaces the whole shape, its values and text included, with what `msShapeFromWKT`
  makes of the text through GEOS: a point shape of one point for a `POINT`, one part of all the
  points for a `MULTIPOINT`, one part per line for a `LINESTRING` or a `MULTILINESTRING`, one part
  per ring, the exterior first, for a `POLYGON` or a `MULTIPOLYGON`, and a line shape of the parts
  of every member for a `GEOMETRYCOLLECTION` (`msGEOSGeometry2Shape` in `src/mapgeos.c`). An
  inner ring that GEOS does not call a ring (`GEOSisRing`) is left out, the exterior kept whatever
  it is: one that is not simple, that passes through a point twice on its way back to its start,
  a point repeated in a row counting once. Its edges in a row meet only at the vertex they share,
  without doubling back, and no other two meet at all; a ring that collapses to a point or to one
  edge is not simple. The frontend decides it exactly, with a sweep of the ring's edges in
  O(n log n) of its vertices. The frontend reads the WKT with a reader of its own that
  re-implements the grammar GEOS's `WKTReader` accepts, no GEOS code: type names and `EMPTY` in
  any case, a type matched by its start, numbers as `strtod` reads a whole token, a
  `MULTIPOINT`'s points with or without their parentheses, a line of at least two points, a ring
  closed and of at least four, and nothing after the geometry. It refuses, as a load error at the
  `WKT`, what that reader does not read, an empty geometry or member, Z and M coordinates, a
  `LINEARRING`, which `msGEOSGeometry2Shape` does not convert, a number that is not finite or that
  only `strtod` reads, such as a hexadecimal one, and collections nested more than 32 deep. The
  error quotes the first 80 characters of the WKT.

A layer with features is drawn from them alone, whatever its `DATA` or `CONNECTION` says, unless
its `CONNECTIONTYPE` is `GRATICULE` (`msInitializeVirtualTable` in `src/maplayer.c`); the engine
reads them as the layer's source and reports a source a host binds to such a layer as not read.
The CLI binds no shapefile to it, so its `DATA` is not read. It delivers the features in the order
they were written, each whose bounds meet the extent drawn, edges included
(`msINLINELayerNextShape`, `msRectOverlap`); a feature without points has the bounds `-1 -1 -1
-1` that `msInitShape` gives it. A point shape of one point is delivered as a point and any other
as a multipoint, a line shape of one part as a line and any other as a multi-line, and a polygon
shape as one polygon of all its parts, as a shapefile's records are.

**Item names.** A feature's values are named by the layer's first `PROCESSING` whose key is
`ITEMS`, without regard to case, followed by `=` or `:` (`msLayerGetItems`, GDAL's
`CSLFetchNameValue`), wherever it stands in the layer, split at `,` with a run of commas one
separator (`msStringSplit`). Each name takes the value at its place, `""` where the feature has
fewer values (`msINLINELayerNextShape`); a value past the names is dropped, and without the key a
feature has no properties. An expression or a binding finds a name without regard to ASCII case,
the first of two that match (`msLayerGetItemIndex`). `PROCESSING "ITEMS=…"` of a layer without
features, and every other `PROCESSING` key, is not yet supported and is ignored with a warning.

**Not carried:** the `TEXT` as a label's text; the layer's `GEOMTRANSFORM` and `TRANSFORM` on
inline features; and a `FEATURE` keyword inside a `FEATURE`, which `loadFeature` skips (`case
(FEATURE): break;`, for features loaded from a string).

**Known divergences.**

- MapServer names an inline layer's values by the items it binds, not only by `ITEMS`:
  `msLayerWhichItems` takes every item of `ITEMS` and appends each item an expression or a
  binding refers to that `ITEMS` lacks (`buildLayerItemList`), so such an item takes the value at
  its place after the names, `""` where the feature has none, and without `ITEMS` the items
  referred to take the values in the order they are referred to. The frontend names values by
  `ITEMS` alone: a name it lacks is absent, as is every name without `ITEMS`.
- An `EMPTY` geometry, which MapServer loads for most types as a shape of an empty part, is a
  load error here.
- A `LINEARRING` member of a `GEOMETRYCOLLECTION`, which MapServer skips
  (`msGEOSGeometry2Shape` converts no linear ring), is a load error here, as a `LINEARRING` alone
  is in both.
