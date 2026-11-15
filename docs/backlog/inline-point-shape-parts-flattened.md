# The parts of an inline point shape are flattened into one multipoint

**Observed:** fidelity ticket 08, circle layers (2026-11-15). The in-memory source of a layer's
inline features hands a point shape of several `POINTS` blocks to the port as one
`Geometry::MultiPoint` of all their points. MapServer keeps each block as a part of the shape.
`circleLayerDrawShape` (`src/mapdraw.c:1988`) draws a circle only of a shape of exactly one part of
two points. So a circle layer's feature of two `POINTS` blocks of one point each draws a circle in
Forager and nothing in MapServer. Every other source delivers a point shape of one part, as a
shapefile's multipoint record is.

**Where:** `crates/engine/src/inline.rs`, `geometry`; `crates/symbolize/src/circle.rs`, `circle`.

**What a change weighs:** the port's `Geometry` has no multipoint of several parts. A
`Geometry::Collection` of each part would keep them, but point layers would then see a collection
where they see a multipoint today: the budget of markers counts a collection's members as shapes
of their own, and the reprojection of a point shape leaves out each member apart. No msautotest
case of the scope is known to hit it.
