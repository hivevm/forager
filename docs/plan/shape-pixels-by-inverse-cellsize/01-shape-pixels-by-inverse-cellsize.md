# Ticket: Lines, polygons and hatches reach pixels as msTransformShape computes them

## What to build

The engine puts a line's or polygon's vertices, and a hatch's rings, at pixels through
`View::to_pixel`, which divides by the cellsize (`crates/engine/src/draw.rs:249`, the `pixels`
closure, and `:327`, `hatch_rings`' `to_pixels`). MapServer's AGG renderer defaults to the transform
mode `MS_TRANSFORM_SIMPLIFY` (`mapagg.cpp:1420`); `msImageStartLayer` (`src/mapdraw.c:4201-4221`)
keeps that default for a layer without the `PROCESSING` key `APPROXIMATION_SCALE`, and sets
another mode only where the key is given. `msDrawShape` (`src/mapdraw.c:2367`) transforms a line
or polygon shape, a hatched one included, before any style draws it (2560, 2570). In that mode
`msTransformShape`
(`mapprimitive.cpp:1065-1084`) calls `msTransformShapeSimplify` (948-1061), which multiplies by
`inv_cs = 1.0 / cellsize` (953) with `MS_MAP2IMAGE_X_IC_DBL` and `MS_MAP2IMAGE_Y_IC_DBL`
(`mapserver.h:473`), for a line's vertices (958-1001) and a polygon's (1002-1040). The quotient and
the product differ in the last bit; for a label's point the difference moved a marker offset across
an `int` cut (#159). Move these paths onto `View::point_to_pixel`.

The ticket covers layers without `APPROXIMATION_SCALE`: the style IR carries no `PROCESSING` key of
the kind, so every layer the engine draws is one. It moves the arithmetic only;
`msTransformShapeSimplify`'s dropping of vertices less than a pixel from their predecessor is not
part of it. `View::to_pixel` stays: it is public through `crates/engine/src/lib.rs:36`, and the
tests at `crates/engine/src/extent.rs:109-131` use it, so its removal is a change of the engine's
interface of its own.

## Acceptance criteria

- [ ] Given a line vertex whose quotient and product by the cellsize differ by one ulp, such as
  `x = 0.3` on a view of cellsize `0.1`, when it is drawn, then its pixel coordinate is the
  product's, `3.0`, stated in the test with `mapprimitive.cpp:953-988` cited.
- [ ] Given the same vertex in a polygon's ring, when it is drawn, then the same holds, with
  `mapprimitive.cpp:1002-1040` cited.
- [ ] Given the same vertex in a hatched polygon's ring, when its hatch is drawn, then the same
  holds.
- [ ] Given the harness, when it runs, then every case holds its listed outcome and `labels-hold`
  mark; a reference image of Forager's that moves is re-blessed with the move stated in the commit.

## Read first

- `msTransformShapeSimplify` and `msTransformShape`, `mapprimitive.cpp:948-1084`;
  `mapagg.cpp:1420`; `msImageStartLayer`, `src/mapdraw.c:4201-4221`, at the pin.
- `crates/engine/src/extent.rs` (`to_pixel`, `point_to_pixel`), `crates/engine/src/draw.rs`.
- #159, which moved the label cache's point onto `point_to_pixel`.

## Blocked by

- None (can start immediately)

## Mode

unattended: engine tests whose values come from the C arithmetic decide each criterion; the change
stays in `crates/engine/src/draw.rs`, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, port, public
interface or style document format, only the arithmetic by which the engine places what it already
draws. Its branch counts against ADR-0007's cap of three open agent branches. A wrong result shows
as a pinned pixel coordinate, a case outcome, or a `labels-hold` mark that moves, and is undone by
the revert; the tests citing the C prove it right. Stop: if any case's listed outcome or
`labels-hold` mark moves, if a reference image of Forager's moves by more than one level per
channel, or past two hours, leave the state here and hand off.
