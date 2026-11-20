# A buffer vanishes where a line or a point is finer than i_overlay's grid, and some run long

**Observed:** revision-2026-11-16 ticket 19 (2026-11-17), by the reviews of its two floors, with
probes run against `buffer` in `crates/symbolize/src/geometry.rs` in a release build. All but the
third were found with the base before ticket 19 too; the third in part. No floor of a line's
distance reaches them: the tiny part and the far line are finer than the grid itself, which a
larger distance only coarsens:

- the line `(-1e200, 0) (1e200, 3e199) (1e200, 0)` returns no polygon buffered by `1e-300` or by
  `1e198`, so it vanishes from the drawing. The same line at `1e150` keeps one, and so do squares
  and points at `1e200`. It looks like the overflow counterpart of ticket 18's underflow;
- the multiline `[(0, 0) (1, 0)], [(0.5, 0.5) (0.5 + 1e-12, 0.5)]` buffered by `0.01` or `0.1`
  returns only the first line's outline: the second part's segments are shorter than the step of
  the grid i_overlay snaps the joint shape to, so the part is dropped where GEOS draws its disc;
- a line far out on one axis and finer than the grid on the other vanishes: `(0, 1e12)
  (1e-10, 1e12)` returns no polygon at its floor, `0.909`, and at any distance above it. By `1e-3`
  it kept one before ticket 19, whose floor of the reach raises the distance and with it the grid.
  It needs an extent below some 2^-70 of the reach, `1e-20` at `2e7`, so it does not occur in a
  map's data. A multiline at `(-2124.5, -3.6e12)` with parts some `1.2e-6` apart vanishes the same
  way;
- a multipoint buffered by a distance near the grid's step vanishes: `(0, 0) (1, 0)` returns no
  polygon by `1e-12`, `1e-10`, or `3e-10`, and two by `1e-9`. A single point and a polygon's ring
  keep theirs. None loops;
- one random self-crossing closed line of 388 points takes some 43 seconds a buffer at any
  distance. A cluster of 260 points within `1e-7` and one far point takes 43 seconds at `2^-10` of
  its size, and returns 13 polygons at `2^-8` and 67 at `2^-4` where one connected polygon is
  expected. The charges of Q-9 stop neither: they bound the points of the arcs, not i_overlay's
  overlay of the stroke.

**Where:** `buffer` in `crates/symbolize/src/geometry.rs`, the stroke and the overlay of i_overlay
4.5.2 (`src/mesh/stroke/`, `src/core/`), which geo's `buffer_with_style` calls.

**What a change weighs:**

- **The huge line:** scaling it down by a power of two, as ticket 18 scales a tiny one up, may keep
  it; whether the overflow lies in i_float's adapter or in the stroke is not found yet.
- **The far line and the multipoint:** a floor of the reach measured on each axis apart, or none
  where the line's extent lies below the grid it would set, keeps the far line; a floor of the
  multipoint's distance as ticket 19's of a line's keeps the multipoint. Each moves a buffer off
  GEOS's further.
- **The tiny part:** stroking each part of a multiline on its own and joining the results keeps it,
  at the cost of a union geo would otherwise not run.
- **The long overlay:** a charge for the overlay needs its cost known first, from a profile of
  i_overlay's overlay on such lines; the i_overlay 9 that geo is to take up
  (REN-i-overlay-shared-with-geo) may change it.
