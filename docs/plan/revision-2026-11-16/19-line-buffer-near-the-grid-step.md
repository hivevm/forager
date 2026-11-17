# Ticket: A line buffered by a distance near i_overlay's grid step neither loops nor vanishes

## What to build

The fourth 300-second run of ticket 01's render fuzz target, after ticket 18's fix, ended a few
seconds in with `libFuzzer: out-of-memory (malloc(3187671040))`. The stack runs from
`forager_engine::transformed` through `layer_transform::transformed` and `Eval::shape` into
`forager_symbolize::geometry::buffer`. From there it enters geo's `buffer_with_style` for a
multiline, i_overlay 4.5.2's `stroke`, and `OverlayGraph::find_contour`
(`i_overlay-4.5.2/src/core/extract.rs:181-210`). A print in `geometry::buffer`, not committed,
decoded the input: the line `(2.177167665407e-311, 4.329103712692384e-304)
(5.908302005e-315, 0)` buffered by `5.43230922487e-312`. Ticket 18 scales it by 2^1008 to a line
some `0.55` long buffered by about `1.25e-8` of its length, which is the case of the backlog entry
this ticket replaces.

The cause is a line's buffer whose width is a few steps of i_overlay's grid, about 2^-30 of the
shape's size. There the stroke's graph is left inconsistent. `find_contour` walks it until it
returns to the node it started from, pushing each point onto the contour. When the walk never
returns, the contour grows until an allocation fails, and the process aborts. The charges of Q-9
do not stop it, since they bound the points of the arcs, not this walk. A throwaway probe outside
the repository, of i_overlay 4.5.2's `stroke` with geo's style on the line `(0, 0) (1, 0)`, shows
the band:

- by `1e-8` and by the fuzz line's ratio, the walk allocates gigabytes;
- by `3e-9`, `1e-9`, `3e-10`, `1e-10`, and `1e-12`, it returns no polygon, so a positive distance
  makes the line vanish from the drawing;
- by `3e-8` it returns one polygon;
- in a build with debug assertions, a closed ring of 200 points and radius `0.3` buffered as a
  line by `1e-9` panics at `debug_assert!(top.is_direct())`
  (`i_overlay-4.5.2/src/core/extract.rs:406`), as the review of ticket 18 found.

i_overlay 9.1.0, which `render-agg` takes, does not loop on these lines, but it too returns no
polygon below about `1e-9`. A polygon's outline and a point's circle run through other code, and
the probes of ticket 18 found no such band for them.

On the maintainer's word (2026-11-17), `geometry::buffer` raises the distance of a line's buffer
to a floor, the larger of two:

- 2^-24 of the size of the lines' joint bounds, the larger of their width and height, since geo
  strokes the parts of a multiline as one subject. That lies 64 or more steps above i_overlay's
  grid. The probe swept 600 random lines of 2 to 31 points (random, near collinear, and a tiny
  cluster with one far point) at 2^-24, 2^-22, and 2^-20 of their size: each returned a polygon
  and none looped. Below it, at 2^-26 and finer, many aborted.
- 2^-40 of the lines' reach, the largest magnitude of their coordinates. Far from the origin the
  coordinates are coarser than the grid: the review of the first floor found a zigzag 0.1 across
  at `(2e7, 2e7)`, as in Web Mercator's metres, aborting when buffered by `1e-9` or floored to 2^-24
  of its size, and a line 1 long at `1e12` aborting when buffered by up to 16 ulps of its
  coordinates, about 2^-47 of its reach. With the second floor the probe's lines at `2e7`, `1e12`,
  and `1e15`, of sizes `1e-3`, `0.1`, and `1`, returned a polygon each, but those whose points all
  coincide in f64, which geo buffers as a point.

The raised buffer still runs through geo's `buffer_with_style`, as ADR-0041 decides. It differs
from GEOS's buffer by at most the floor, far below a pixel at any size a map draws, and
`docs/semantics/style-symbol.md` names the difference where it describes `buffer`. A distance not
above 0 still returns nothing, as today. Points and polygons are not raised.

The review of the first floor found three older failures of a line's buffer that no floor of its
distance reaches, filed in `docs/backlog/line-buffer-vanishes-or-runs-long.md` rather than here: a
line some `1e200` across vanishes at any distance, a part of a multiline shorter than the joint
grid's step is dropped, and some self-crossing lines of a few hundred points take tens of seconds
past the charges. The Mode's stop on a line that loops or vanishes at or above the floor is meant
for failures of the grid step, not for these.

The charges of `buffer` do not change: `arcs` counts the points of the round joins and caps from
the angle of each turn, not from the distance, and the other charges count the given points.

The 122-byte input, for `cargo fuzz run render <file>` on the target of
`ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths`:

```text
40 03 01 37 ff ff ff 7e ff 00 3d 00 00 00 00 00
00 00 00 00 73 02 08 73 00 40 07 00 00 00 08 47
f6 00 00 00 00 00 00 00 00 00 00 00 00 00 00 00
00 00 00 00 00 00 00 09 00 40 ff ff ff ff ff ff
ff c5 00 ff ff ff ff ff 70 6f 7a 69 70 69 78 6d
61 70 2d 70 6f 69 6e 74 6e 74 20 2b 00 7f 7f 85
7f 7f 7f 7f 86 80 ff ff 01 04 00 00 00 00 00 01
00 00 f3 00 47 47 47 47 47 00
```

## Acceptance criteria

"Covers" means that every vertex of the input lies inside the result, or within a millionth of its
extent of the result's boundary. "Bounded" means that every vertex of the result lies within the
input's bounds grown by the raised distance and a millionth of its extent.

- [ ] Given the fuzz line above buffered by `5.43230922487e-312`, when `geometry::buffer` runs in a
  test, then it returns, with polygons of only finite coordinates that cover the line and are
  bounded. Before the change the test aborts on a failed allocation.
- [ ] Given the line `(0, 0) (1, 0)` buffered by `1e-8`, `3e-9`, `1e-9`, and `1e-12`, and the
  closed ring of 200 points and radius `0.3` buffered as a line by `1e-9`, when `geometry::buffer`
  runs, then each returns polygons of only finite coordinates that cover the line and are bounded.
  Before the change the first aborts, the others return no polygon, and the ring panics in the
  test build.
- [ ] Given the zigzag above buffered by `1e-300` and by `1e-9`, and the line above at `1e12`
  buffered by 8 ulps of its coordinates, when `geometry::buffer` runs, then each returns polygons
  that cover it and are bounded. Under the floor of the size alone each aborts.
- [ ] Given a line buffered by a distance at its floor or above, and the buffer tests of
  `crates/symbolize/src/geometry/tests.rs`, when they run, then they pass unchanged. These include
  GEOS's references, the charges that the tests' `Recorder` records, and ticket 18's tests. The
  msautotest cases with `buffer` pass unchanged too.
- [ ] Given a copy of ticket 01's target with this change merged, not that ticket's worktree and
  nothing committed, when the input above is run by `cargo fuzz run render`, then it runs clean.
- [ ] Given `docs/semantics/style-symbol.md`, when it describes `buffer`, then it names both floors
  of a line's distance and that GEOS keeps the thinner buffer.

## Read first

- Touches: `crates/symbolize/src/geometry.rs` (`buffer`), `crates/symbolize/src/geometry/tests.rs`,
  `docs/semantics/style-symbol.md`
- ADR-0041 (geometry through geo: `buffer` is geo's `buffer_with_style`), ADR-0042 (i_overlay in
  render-agg and geo); `docs/SPECIFICATION.md` Q-3 and Q-9; the `Charge` trait of `geometry.rs`,
  `arcs`, `magnification`, and the tests' `Recorder`;
  `crates/symbolize/src/geometry/geos-buffer.txt`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the walk it ends is unbounded memory, which Q-9 bounds and ADR-0006 keeps from
unattended work, and the floor is a deviation from GEOS the maintainer chose. Stop if:

- a charge, or an assertion on a `Recorder`, has to change;
- the fix leaves geo's `buffer_with_style` for some inputs;
- a GEOS reference, an msautotest case, or any other existing test has to change;
- a line at or above the floor still loops or vanishes, or a polygon or a point is found to.
