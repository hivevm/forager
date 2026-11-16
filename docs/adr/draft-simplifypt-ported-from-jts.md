# ADR-draft-simplifypt-ported-from-jts: `simplifypt` is computed in `symbolize` by a port of JTS's `TopologyPreservingSimplifier`

- **Status:** 🟢 accepted
- **Date:** 2026-11-16
- **Deciders:** Markus Brigl
- **Applies to:** the code of `simplifypt` in the geometry module of `crates/symbolize` and its callers in `crates/symbolize/src/layer_transform.rs` and the drawing of a class style's tree, the notice of JTS on the files it ports, and the unit tests that hold coordinates taken from GEOS for it

## Context

MapServer at the pin computes `simplifypt(s, n)` by `msGEOSTopologyPreservingSimplify`
(`src/mapgeos.c:946`). It calls `GEOSTopologyPreserveSimplify_r`, GEOS's
`TopologyPreservingSimplifier`. That is a Douglas-Peucker that refuses to flatten a section where
the new segment would cross the rest of the geometry. The noise run builds MapServer against GEOS
3.12.1, the reference of [ADR-0041](0041-geometry-through-geo.md).

ADR-0041 computes `simplify` by a Douglas-Peucker ported from JTS, in a geometry module of
`symbolize`. Its *Out of scope* keeps `simplifypt` a `not_drawn` reason, and its *Context* leaves
it not drawn until a later decision. This ADR is that decision. On 2026-11-16 the maintainer
chose the first option of the backlog entry `docs/backlog/simplifypt-topology-preserving.md`:
a port of JTS's `TopologyPreservingSimplifier`.
[ADR-0043](0043-transform-tree-with-simplify-and-buffer.md) keeps `simplifypt` in the closed list
of reasons. Its successor,
[ADR-draft-transform-tree-with-simplifypt](draft-transform-tree-with-simplifypt.md), adds the node.

**The case.** The style `simplifypt([shape],50)` of `renderers/geomtransform-vertex.map` works on
the three lines of `misc/data/testlines.shp`, in pixels. Ticket 10 of the fidelity plan measured
that this style is all that the case `geomtransform-vertex.png` still lacks.

**How JTS simplifies with topology** (`org.locationtech.jts.simplify` at JTS's `master`):

- Each line of the geometry, each ring included, is a tagged line. Its segments enter an input
  index. Lines are simplified one after the other.
- A section is flattened to the segment between its ends where all of these hold:
  - its farthest point lies within the tolerance;
  - the line keeps its minimum size: 2 points for an open line, 4 for a closed one;
  - the new segment meets no segment of the output index, and no segment of the input index
    outside the section, in an interior point or as an equal segment;
  - the new segment does not jump another component. A ray from one point of that component
    crosses the section and the new segment an even number of times each, or an odd number each.
    The point is the first of the component's result, or its second point before it is simplified.
- Otherwise the section is split at its first farthest point, by recursion.
- A flattened section leaves the input index, and its new segment enters the output index.
- After a ring, its first point is dropped where it lies within the tolerance of the segment
  between its neighbours. The ring must hold more than 4 coordinates before the drop. Where the
  first point lies on the new segment's line, the drop is valid at once. Otherwise the new segment
  passes the same tests. An open or a closed line keeps its first point.
- No ring is removed and nothing is repaired.

**GEOS 3.12.1 differs from JTS.** JTS's simplifier took five fixes between 2022 and 2024. GEOS
3.12.1 carries one of them.

| Fix | JTS | GEOS |
|---|---|---|
| a section flattened onto an equal segment is refused | #925, 2022-11 | 3.13.0 |
| a ring's first point dropped, checked against the indexes (GH-784, GH-986) | #1013, 2023-11 | 3.12.0, 3.12.1 |
| a flattening that jumps another component is refused | #1024, 2023-12 | 3.13.0 (#1012) |
| the indexes updated when a ring's first point is dropped | #1059, 2024-06 | 3.13.0 (#1110) |
| the jump test takes a point of the component's result | #1096, 2024-11 | 3.13.1 |

JTS 1.20.0, of 2024-08-30, holds the first four. Its `master` of 2026-11-14, commit `722a943c`,
adds #1096 and renames one parameter. Nothing else in the five files of the *Decision* changed.
GEOS 3.12.1 drops a ring's first point without the jump test. It deletes the replaced segment
while the output index still holds it. JTS takes the lines in the order of a `HashMap`. GEOS takes
them in the order of the geometry since 3.12.0 (GH-718).

The maintainer's order named JTS 1.20.0. This ADR takes `master`, for three reasons. #1096 fixes a
defect of 1.20.0's jump test. With it, the port reproduced GEOS 3.12.1 on more shapes than 1.20.0
did (*Measurements*). And ADR-0041's Douglas-Peucker follows JTS's `master` too.

**What the result depends on.** A query of an index only answers whether some segment fails a
test. So the result does not depend on the index, only on the order of the lines and the tests.
Each test is an orientation, a comparison of coordinates, or `Distance.pointToSegment`. JTS
computes an orientation in double-double arithmetic after a filter. geo's `RobustKernel` computes
its sign exactly, by the `robust` crate that geo already brings.

**Measurements** (2026-11-16, `.scratch/simplifypt-research/`, outside the workspace). A
prototype in Rust ported JTS's `master` as this ADR decides: lines in the geometry's order, an
explicit stack, exact orientation, and no spatial index. Other modes of it took JTS 1.20.0, and
GEOS 3.12.1's differences. GEOS 3.12.1 ran through the C program of ADR-0041. The pixels were
computed as `msAdjustExtent` and `msTransformShapeSimplify` compute them. Every coordinate was
compared as a double.

| Input | The port against GEOS 3.12.1 |
|---|---|
| `testlines.shp` in the pixels of `geomtransform-vertex.map`, at 1, 2, 5, 10, 20, 40, 50 | bit-identical, all 3 lines |
| `testlines.shp` in map units, at the same tolerances | bit-identical |
| `world_testpoly.shp` in map units and in the pixels of `polygon-as-line-offset.map`, at the same tolerances | bit-identical, all 4 polygons |
| 272 random valid polygons with 6 decimals, 288 on an integer grid, at 1, 2, 5, 10, 20, 40 | 3,357 of 3,360 bit-identical to a result of GEOS run on each polygon alone, three runs each |
| a noisy ring of 1,000 to 100,000 points, a zigzag of 10,000, a wiggling line of 100,000, a shell with 100 and 1,000 holes, at 5 and 50 | bit-identical |
| 200 shapes of 2 to 5 lines in bands that do not overlap, at 1, 3, 5, 10, 20, 40 | bit-identical |
| 200 shapes of a wavy line with short lines inside its bends, at 2, 5, 10, 20, 40 | 0, 0, 10, 62, and 141 of 200 differ |
| 200 shapes of 2 to 5 lines that cross each other, at 1, 3, 5, 10, 20 | 0, 3, 14, 26, and 37 of 200 differ |

The 3 polygons are on the integer grid at 40. GEOS 3.12.1 drops the shell's first point there,
and the hole leaves the shell. GEOS's own `isValid` refuses each of the 3 results; the port's are
valid. Every difference among the lines is the jump test: without it, the port reproduced GEOS
3.12.1 on all of them. The smallest such shape is
`MULTILINESTRING ((0 0, 10 10, 20 0), (9 4, 11 4))` at 20. GEOS 3.12.1 flattens the bend to
`(0 0, 20 0)`, and the short line changes sides. The port keeps the bend. JTS 1.20.0 differed
from GEOS on 11 of the wavy shapes at 2 and at 5, where `master` differs on none. A mode with
GEOS 3.12.1's differences reproduced it on every shape measured.

GEOS 3.12.1's results also vary. Run over the whole file, it gave another result than alone for
1 to 4 of these polygons at 10, 20, and 40. Five runs of the same file gave two different outputs.
One polygon alone gave two different outputs in eight runs. It is the deleted segment that the
output index still holds. The case's shapes are single lines without a ring. Neither the jump test
nor the deleted segment reaches them.

**Cost.** A prototype without a spatial index examines every live segment on each query of an
index. The counts:

| Input | Tolerance | Points scanned | Segments examined | Component tests | Time, native |
|---|---|---|---|---|---|
| a ring of 10,000 points | 50 | 212,257 | 24,561,322 | 1,581 | 45 ms |
| a ring of 20,000 points | 1 | 505,476 | 4,890,858 | 217 | 17 ms |
| a ring of 20,000 points | 50 | 499,038 | 111,226,623 | 1,866 | 0.16 s |
| a ring of 100,000 points | 50 | 3,628,881 | 3,052,481,130 | 4,290 | 5.3 s |
| a zigzag of 10,000 points, each kept | 50 | 49,984,440 | 9,999 | 36 | 0.35 s |
| a wiggling line of 20,000 points | 1 | 1,918,547 | 79,167,316 | 19,080 | 0.17 s |
| a wiggling line of 30,000 points | 1 | 3,767,276 | 177,993,205 | 28,705 | 0.41 s |
| a wiggling line of 100,000 points | 1 | 20,401,817 | 1,966,033,603 | 95,802 | 5.0 s |
| a wiggling line of 100,000 points | 5 | 19,935,824 | 168,122,602 | 103,254 | 0.33 s |
| a wiggling line of 100,000 points | 50 | 19,764,247 | 53,713,925 | 101,061 | 0.21 s |
| a shell with 1,000 holes, 12,001 points | 50 | 32,277 | 31,756,443 | 4,472,831 | 92 ms |
| a shell with 2,000 holes, 24,001 points | 50 | 68,627 | 134,355,972 | 19,461,618 | 0.35 s |
| a shell with 5,000 holes, 60,001 points | 50 | 185,361 | 862,718,495 | 124,248,559 | 2.2 s |

A ring costs most at a large tolerance, a long open line at a small one. With rstar's R-tree as
the index, the ring of 100,000 points at 50 examines 24,608,377 nodes and segments in 0.52 s. The
wiggling line of 100,000 points at 1 examines 3,261,214 in 0.28 s. GEOS 3.12.1 took 1.4 s for the
ring, with its output written. It took 0.06 s for the wiggling line of 20,000 points at 1,
0.11 s for 30,000, and 0.66 s for 100,000, each the median of three runs with its output written.
Q-9 lets one layer's transforms visit 100,000,000 points. Without
an index, a wiggling line of about 22,000 points at 1 passes that bound alone. So does a ring of
20,000 points at 50, and a shell with 2,000 holes.

**Bundle.** A wasm32 build of the prototype alone, at the `wasm` profile and gzipped, against an
empty module: +9,073 B without an index, +29,050 B with rstar. The `robust` crate is counted in
both. geo already links it into the bundle, so the growth in the bundle is at most these figures.

**Forces.**

- Q-4 and G-6: the case passes by a raster metric.
- Q-1: no C. Q-6: permissive licences, and no code of GEOS in the tree.
- Q-2 and G-9: the same bits on every target.
- Q-3: no input crashes the render, the browser's included.
- Q-9: the simplification visits points, and its tests visit segments and components. Q-9 counts
  points visited. This ADR reads a segment examined and a component compared as one point visited
  each.
- Q-5: the bundle stays under 3 MB.
- Simplicity: the smallest port that reproduces the reference.

## Decision

We will compute `simplifypt` in the geometry module of `crates/symbolize` by a port of JTS's
`TopologyPreservingSimplifier`, at its `master` of 2026-11-14.

- **Place.** The port lives beside ADR-0041's Douglas-Peucker. A layer's transform and a class
  style's tree call the same function. No crate of the workspace is added, and no dependency.
- **Sources.** The port must follow these files of JTS at commit `722a943c`:
  - `TopologyPreservingSimplifier.java`, `TaggedLinesSimplifier.java`,
    `TaggedLineStringSimplifier.java`, `TaggedLineString.java`, and `ComponentJumpChecker.java`;
  - the interior intersection of `RobustLineIntersector.java` and `LineIntersector.java`;
  - the crossing count of `RayCrossingCounter.java`;
  - `equalsTopo`, `distance`, and `orientationIndex` of `LineSegment.java`, `pointToSegment` of
    `Distance.java`, and `isRing` of `CoordinateArrays.java`;
  - the bounding boxes the queries and the jump test compare, of `Envelope.java`.
- **Order.** The port must simplify the lines of one shape in the shape's order: each part of a
  line, or each polygon's shell before its holes. JTS's order of a `HashMap` is not ported.
- **No recursion.** The port must keep its pending sections on a stack of its own. It must take
  the left section before the right one, as JTS's recursion does. Each section must carry its
  depth for the minimum size.
- **Orientation.** Every orientation must be the sign of geo's `RobustKernel`. JTS's
  double-double arithmetic is not ported.
- **Distance.** Each distance must be JTS's `Distance.pointToSegment`, computed by `f64::sqrt`,
  as ADR-0041's Douglas-Peucker computes it.
- **Index.** The input and the output index must answer each query by examining every segment
  they hold. The port must not build a spatial index.
- **A section of no number.** Where every distance in a section is no number, its split point is
  its first end. The port must keep such a section whole where it cannot flatten it. JTS and GEOS
  3.12.1 split it again without end there.
- **Reference.** The port must follow JTS's sources, not GEOS's. Each file that ports a JTS source
  must name it and hold JTS's EDL notice, as ADR-0041 asks.
- **Mathematics.** The port must take no transcendental function. It may use `f64::sqrt` and
  IEEE's arithmetic.
- **Bounds.** The port must charge the budget of Q-9 that ADR-0041's functions charge, as points
  visited:
  - the inner points of each section, before it scans them;
  - each segment a query of an index examines;
  - each component the jump test compares, and each segment whose crossings it counts.

**Out of scope:**

- the transform tree's node, which
  [ADR-draft-transform-tree-with-simplifypt](draft-transform-tree-with-simplifypt.md) decides;
- a tolerance that is no number, for which GEOS 3.12.1 flattens every section the topology tests
  allow;
- a tolerance below 0, for which GEOS fails, and MapServer's reaction to that failure.
  `docs/semantics/` writes them down from the C, and the port follows that page;
- the shapes MapServer hands to GEOS and takes back, which `docs/semantics/` writes down as for
  `simplify`;
- `simplify` and `buffer`, which ADR-0041 decides.

## Alternatives considered

- **JTS 1.20.0, as the maintainer's order named it** — a release. Its jump test takes a
  component's second point even after the component is simplified, which #1096 fixed. It differed
  from GEOS 3.12.1 on 11 of 200 wavy shapes at 2 and 5, where `master` differs on none.
- **A port of the JTS of GEOS 3.12.1** — JTS at commit `0640925f` of 2023-11-06, after #1013,
  with #925 taken out again. The measured mode reproduced GEOS 3.12.1 on every
  shape, the lines that jump a component among them. It is no release of JTS. It ports four
  defects JTS has fixed since: a line jumps another and changes sides, a hole leaves its shell, an
  equal segment passes, and a replaced segment stays in the output index. Where GEOS 3.12.1 reads
  that segment after deleting it, its results vary with its heap, and no port follows them. GEOS
  3.13.1 follows JTS's `master`, as its notes and source read. A runner of a later Ubuntu would
  build MapServer against such a release.
- **JTS's `Quadtree` as the index, ported** — the index JTS and GEOS build. Its queries examine
  fewer segments, so a larger polygon passes Q-9. It is about 600 more lines of Java to port, and
  the result does not depend on it.
- **rstar's R-tree as the index** — measured: the ring of 100,000 points examines 25 million
  nodes and segments, where the port without an index examines 3,052 million. rstar 0.12 is already
  in `Cargo.lock` through geo; it would be a direct dependency of `symbolize`. It adds about
  20 KB to the gzipped bundle. No case in scope needs it.
- **JTS's double-double orientation, ported** — the arithmetic JTS computes. It would agree with
  the exact sign except where double-double arithmetic rounds wrongly. It is about 100 more lines
  of code, where geo already brings an exact sign.
- **The recursion of JTS** — the shortest port. A line that keeps each point nests it once a
  point, as geo's `Simplify` did. That overflowed the stack at 1,000 points in wasmi (ADR-0041).
- **geo's `SimplifyVwPreserve`** — no code of Forager's own. It is Visvalingam-Whyatt, whose
  tolerance is an area, and it differs from GEOS on 4 of the case's 6 line results.
- **A successor of ADR-0041 that adds `simplifypt`** — one record for the geometry module.
  ADR-0041's *Out of scope* keeps `simplifypt` a `not_drawn` reason. That names what ADR-0041 leaves
  undecided, and the reason itself is ADR-0043's to remove. No clause of its *Decision* changes:
  its rules on the reference, the mathematics, and the bounds bind its own steps, and this ADR
  states its own. A successor would restate ADR-0041 whole to add a function beside it.
- **Leave `simplifypt` not drawn** — no code. The case `geomtransform-vertex.png` keeps failing.
- **A port of GEOS's C++** — rejected: LGPL-2.1, against Q-6.
- **The `geos` crate** — rejected: it binds GEOS's C library through `geos-sys`, against Q-1.

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapgeos.c` (`:946`) and `src/mapprimitive.cpp`
  (`msTransformShapeSimplify`, `:948-1063`), read 2026-11-16 from `target/ms-src`.
- JTS, github.com/locationtech/jts at the tag `1.20.0` and at `master`, commit `722a943c`, read
  2026-11-16:
  `modules/core/src/main/java/org/locationtech/jts/simplify/` (the five files of the
  *Decision* and `LineSegmentIndex.java`), `algorithm/RobustLineIntersector.java`,
  `algorithm/LineIntersector.java`, `algorithm/RayCrossingCounter.java`,
  `algorithm/CGAlgorithmsDD.java`, `geom/Envelope.java`, and `LICENSES.md`.
- JTS's history of the simplifier: #925, #1013, #1024, #1059, #1096, read 2026-11-16 through
  GitHub's API.
- GEOS 3.12.1, its release tarball and `NEWS.md` (GH-718, GH-784, GH-986), built and called
  2026-11-16. Its sources were read to learn where it differs from JTS; they are not a reference
  for Forager's code. GEOS's commits #1012 and #1110, and `NEWS.md` and
  `TaggedLineStringSimplifier.cpp` at 3.13.1, read 2026-11-16 through GitHub's API.
- geo 0.33.1, `algorithm/kernels/robust.rs`; rstar 0.12.2, `rtree.rs` and
  `algorithm/selection_functions.rs`.
- [ADR-0041](0041-geometry-through-geo.md), [ADR-0034](0034-wasm-renders-compared-in-wasmi.md),
  [ADR-0011](0011-crate-supply-chain.md).

## Consequences

- Positive: the style `simplifypt([shape],50)` of `geomtransform-vertex.map` draws GEOS's points
  bit for bit, as measured, and the case can pass. Single lines and polygons matched GEOS 3.12.1
  on every result where GEOS's own result is valid and stable.
- Positive: the port refuses a flattening that makes a hole leave its shell or a line jump
  another, where GEOS 3.12.1 does not. Its results do not vary from run to run. It follows GEOS
  3.13.1, as that release's notes and source read; 3.13.1 was not run.
- Positive: no line overflows the stack, and every test is exact or computed by IEEE's arithmetic.
- Negative / trade-offs:
  - where GEOS 3.12.1 lets a flattening jump another line of the same shape, or a hole leave its
    shell, the port keeps more points than MapServer. Of 200 shapes built to provoke a jump,
    141 differed at 40;
  - without a spatial index, the queries examine the square of a shape's segments in the worst
    case. A wiggling line of about 22,000 points at a tolerance of 1, or a ring of 20,000 points
    at 50, passes the bound of Q-9 on visits alone. Its layer draws nothing, where MapServer draws
    it. GEOS 3.12.1 simplified such a line in about a tenth of a second, as measured;
  - the jump test compares each section with every other component. With the queries, a polygon
    of 2,000 holes passes the bound in the same way;
  - a zigzag of falling amplitude scans about n²/2 points, as ADR-0041's Douglas-Peucker does;
  - Forager owns about 400 more lines of geometry, ported from JTS, as the prototype counts.
- Follow-ups:
  - a spatial index, once real data passes the bound;
  - the semantics page of `simplifypt`, its tolerance below 0 among it;
  - GEOS's references for `simplifypt` taken in the noise run, as for `simplify` (ADR-0041).

## Enforcement

- Unit tests of the geometry module cite this ADR with `Verifies:`. They hold coordinates taken
  from GEOS 3.12.1:
  - the case's three lines in the pixels of `geomtransform-vertex.map` at 50, bit for bit;
  - `world_testpoly.shp` at 20 map units, a ring's first point dropped among them, bit for bit;
  - two lines of one shape where a flattening would cross the other line, bit for bit;
  - a closed line, which keeps its first point and 4 coordinates, bit for bit.
- Unit tests cite this ADR. They hold where the port departs from GEOS 3.12.1, as JTS's `master`
  decides:
  - the shape `MULTILINESTRING ((0 0, 10 10, 20 0), (9 4, 11 4))` at 20 keeps its bend;
  - a polygon whose shell would jump its hole when its first point is dropped keeps the hole
    inside the shell.
- A unit test cites this ADR. A line whose coordinates are no number is simplified without a hang,
  its sections kept whole.
- A unit test cites this ADR and Q-9. It holds the charge of the scanned points, of the segments
  examined, and of the components compared.
- A test renders a layer of `simplifypt` over a zigzag line of several thousand points, each kept,
  natively and through the wasm32 module in wasmi. Both must render without a trap, bit for bit
  alike (Q-3, G-9).
- The conformance list holds `renderers/geomtransform-vertex.map geomtransform-vertex.png` as
  `geometry passes`, rendered alike natively and on wasm32 (G-9).
- `scripts/check-notice.sh` fails on a file under `crates/symbolize` that names a JTS source without
  JTS's notice (ADR-0041).

**Not mechanically decidable:** that the port follows JTS's sources and not GEOS's. Review
compares the port with the files the *Decision* names.
