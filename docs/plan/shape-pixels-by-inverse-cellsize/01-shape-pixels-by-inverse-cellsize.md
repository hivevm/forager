# Ticket: Lines, polygons and hatches reach pixels as msTransformShape computes them

## What to build

The engine puts a line's or polygon's vertices, and a hatch's rings, at pixels through
`View::to_pixel`, which divides by the cellsize (`crates/engine/src/draw.rs`, the `pixels` closure
and `hatch_rings`' `to_pixels`). MapServer's AGG renderer transforms a shape in the mode
`MS_TRANSFORM_SIMPLIFY` (`mapagg.cpp:1420`), so `msTransformShape` (`mapprimitive.cpp:1065`) calls
`msTransformShapeSimplify`, which multiplies by `1.0/cellsize` with `MS_MAP2IMAGE_*_IC_DBL`
(`mapprimitive.cpp:953-988`). The two differ in the last bit; for a label's point the difference
moved a marker offset across an `int` cut (#159). Move these paths onto `View::point_to_pixel`, and
remove `View::to_pixel` once nothing outside its tests calls it.

## Acceptance criteria

- [ ] Given a line vertex whose quotient and product by the cellsize differ by one ulp, when it is
  drawn, then its pixel coordinates are the product's, stated in the test from the C arithmetic.
- [ ] Given the same for a polygon and for a hatch's ring, then the same holds.
- [ ] Given the harness, then every case holds its listed outcome and mark; a reference image of
  Forager's that moves is re-blessed with the move stated in the commit.

## Read first

- `msTransformShapeSimplify`, `mapprimitive.cpp:953-988`, and `mapagg.cpp:1420` at the pin.
- `crates/engine/src/extent.rs` (`to_pixel`, `point_to_pixel`), `crates/engine/src/draw.rs`.
- #159, which moved the label cache's point onto `point_to_pixel`.

## Blocked by

- None (can start immediately)

## Mode

unattended: it touches nothing ADR-0007 keeps with the human (no resource limit, port, public
interface or style document format), its branch counts against ADR-0007's cap of three, and it
changes only `crates/engine`. Stop: if any case's listed outcome or `labels-hold` mark moves, or
past two hours, leave the state here and hand off.
