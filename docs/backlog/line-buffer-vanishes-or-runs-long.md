# A line's buffer vanishes when the line is huge or a part is tiny, and some run long past the charges

**Observed:** revision-2026-11-16 ticket 19 (2026-11-17), by the review of its first floor, with
probes run against `buffer` in `crates/symbolize/src/geometry.rs` in a release build. Each was
found with the base before ticket 19 too; no floor of a line's distance reaches them:

- the line `(-1e200, 0) (1e200, 3e199) (1e200, 0)` returns no polygon buffered by `1e-300` or by
  `1e198`, so it vanishes from the drawing. The same line at `1e150` keeps one, and so do squares
  and points at `1e200`. It looks like the overflow counterpart of ticket 18's underflow;
- the multiline `[(0, 0) (1, 0)], [(0.5, 0.5) (0.5 + 1e-12, 0.5)]` buffered by `0.01` or `0.1`
  returns only the first line's outline: the second part's segments are shorter than the step of
  the grid i_overlay snaps the joint shape to, so the part is dropped where GEOS draws its disc;
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
- **The tiny part:** stroking each part of a multiline on its own and joining the results keeps it,
  at the cost of a union geo would otherwise not run.
- **The long overlay:** a charge for the overlay needs its cost known first, from a profile of
  i_overlay's overlay on such lines; the i_overlay 9 that geo is to take up (ADR-0042) may change
  it.
