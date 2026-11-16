# A layer's `GEOMTRANSFORM` is not drawn on a multipoint

**Observed:** while keeping an inline feature's `POINTS` blocks apart (2026-11-16). The port's
multipoint now keeps a point shape's parts: one for a GeoJSON or a shapefile multipoint, as OGR's
`ogrGeomPoints` and a shapefile's record give it, and one per `POINTS` block of an inline feature.
`Shape::of` in `crates/symbolize/src/layer_transform.rs` still refuses a multipoint, as it did when
the parts were flattened, so a layer with a `GEOMTRANSFORM` draws nothing on one and warns.
MapServer transforms such a shape. Its GEOS-based transforms, such as `simplify` and `buffer`,
take the first part of a point shape (`msGEOSShape2Geometry`, `src/mapgeos.c:365-374`), as the
port of them already does for a point.

**Where:** `crates/symbolize/src/layer_transform.rs`, `Shape::of`;
`docs/semantics/style-symbol.md`, the paragraph on what fails a layer's transform.

**What a change weighs:** mapping a multipoint to a point shape of its parts would likely be
enough, with tests of each transform on a shape of several parts. It changes an outcome from not
drawn to drawn, so the msautotest cases of the scope would show whether any of them moves.
