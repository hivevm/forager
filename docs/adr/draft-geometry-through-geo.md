# ADR-draft-geometry-through-geo: `simplify` and `buffer` are computed in `symbolize`, by a Douglas-Peucker ported from JTS and with the geo crate, and a simplified polygon is repaired as GEOS repairs it

- **Status:** 🟡 proposed
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the geometry module of `crates/symbolize` that computes `simplify` and `buffer` and its callers in `crates/symbolize/src/layer_transform.rs`, `crates/symbolize/Cargo.toml`, `Cargo.lock` for geo and the crates it brings, `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of JTS, and the unit tests that hold coordinates taken from GEOS

## Context

MapServer at the pin computes three shape functions of `GEOMTRANSFORM` through GEOS
(`src/mapgeos.c`):

- `simplify` is `msGEOSSimplify` (`:922`), which calls `GEOSSimplify_r`, GEOS's
  `DouglasPeuckerSimplifier`;
- `simplifypt` is `msGEOSTopologyPreservingSimplify` (`:946`), which calls
  `GEOSTopologyPreserveSimplify_r`, GEOS's `TopologyPreservingSimplifier`;
- `buffer` is `msGEOSBuffer` (`:899`), which calls `GEOSBuffer_r` with 30 segments a quadrant.

The noise run builds MapServer against the runner's `libgeos-dev` (`scripts/build-mapserver.sh`,
`ubuntu-24.04`). That package is GEOS 3.12.1 (packages.ubuntu.com, noble, `3.12.1-3build1`). That
GEOS is the reference the cases are held to.

GEOS is a C++ port of JTS, the Java Topology Suite. GEOS is under LGPL-2.1. JTS is offered under
EPL-2.0 or the Eclipse Distribution License 1.0. The EDL is the New BSD licence, SPDX
`BSD-3-Clause`, which [ADR-0011](0011-crate-supply-chain.md)'s list allows.

**The cases.** msautotest at 7515cccf uses the three functions in two mapfiles
([ADR-0036](0036-typed-geomtransform.md), its table):

- `renderers/geomtransform-vertex.map`: the layer `vertex2` of `simplify([shape],25)`, which works
  on the shapes of `misc/data/testlines.shp` in map units; and three styles, `simplify([shape],10)`,
  `simplifypt([shape],50)`, and `generalize([shape],50)`, which work in pixels;
- `renderers/polygon-as-line-offset.map`: two styles, `buffer([shape], 1)` and
  `buffer([shape], -1)`, on the polygons of `misc/data/world_testpoly.shp` in pixels.

No case simplifies a polygon.

**How GEOS simplifies a polygon.** JTS's `DouglasPeuckerSimplifier` and the output of GEOS 3.12.1
agree on these steps:

- each ring is simplified by Douglas-Peucker, which keeps the first of equally far points;
- a ring of 4 or more coordinates then loses its first point when that point lies within the
  tolerance of the segment between its neighbours (GEOS 3.12.0, GH-773);
- a ring left with fewer than 4 coordinates is removed. A removed shell leaves `POLYGON EMPTY`;
- the result is repaired by a buffer of 0 only when it is not a valid area. A multipolygon is
  repaired as a whole.

A line keeps its end points.

**Measurements** (2026-11-15, in `.scratch/`, outside the workspace). GEOS 3.12.1 was built from
its release tarball, GEOS 3.13.1 taken from Debian. A C program called GEOS as
`msGEOSShape2Geometry` builds the geometry. Rust programs called geo 0.33.1 and i_overlay 9.1.0 on
the same coordinates. Every coordinate was printed with 17 significant digits.

| Input | geo 0.33.1 against GEOS 3.12.1 |
|---|---|
| `testlines.shp`, `Simplify` at 1, 5, 10, 25, 50 map units | bit-identical, all 3 lines |
| the same in the pixels of `geomtransform-vertex.map`, at 10 and 50 | bit-identical |
| `testlines.shp` in pixels, `SimplifyVwPreserve` against GEOS's `TopologyPreserveSimplify` at 10 and 50 | 4 of 6 lines differ: 4 points against 9, 5 against 7, 3 against 8, 2 against 3 |
| `world_testpoly.shp`, `Polygon::simplify` at 10, 20, 40 | 1 to 4 of 4 differ: geo keeps collapsed rings and the first point, and repairs nothing |
| the pipeline below, 272 random valid polygons with 6 decimals, at 1, 2, 5, 10, 20, 40 px | 1,628 of 1,632 bit-identical; the 4 repaired ones each within 1.6 × 10⁻⁷ px, same point count |
| the same pipeline, `world_testpoly.shp` at 1 to 20 map units | bit-identical; one repaired polygon within 6.7 × 10⁻⁸ |
| the same pipeline, 288 random valid polygons on an integer grid, at 1 to 20 px | 0.7 % to 2.8 % differ, each by the rule for equally far points |
| `world_testpoly.shp` in pixels, `buffer` ±1, round joins and caps of π/60 | the same point count, every vertex within 2.5 × 10⁻⁷ px, no coordinate identical |
| `testlines.shp` in pixels, `buffer` 1 and 5 | the same point count, every vertex within 2.3 × 10⁻⁶ px |
| 272 random polygons, `buffer` +3 and −3 px | +3: the same point count, within 3.8 × 10⁻⁵ px; −3: 2 of 272 differ in point count, within 2.1 × 10⁻³ px |

The pipeline is geo's `LineString::simplify` on each ring, then three steps of Forager's own: the
first-point step, the removal of a ring under 4 coordinates, and a repair of an invalid result by
geo's `unary_union`. geo's `Validation` decided validity. It agreed with GEOS's `isValid` on every
one of the 3,053 rough results. The repair by `unary_union` matched GEOS's buffer of 0 on the 7
repaired polygons whose rough result matched. A later measurement of the implementation contradicts
the general claim: on 2,467 small random valid polygons, of 6 to 9 vertices and 2 decimals, at 3, 6,
and 10 units, 40 results were repaired. 33 matched GEOS within 10⁻⁷ with its point count. 7 kept
another lobe of a ring that crosses itself than GEOS keeps, 27 to 54 units away, in either
orientation of the input. No case in scope simplifies a polygon. The equally far points were
counted with a script of both rules: on the integer grid, the rule explains every difference; on 6
decimals it changed nothing.

GEOS 3.12.1 and 3.13.1 gave identical results but one: a repaired polygon at 20 map units, whose
computed vertex differs in its last bit.

**Determinism.** geo's distance from a point to a segment calls std's `f64::hypot`. On
`wasm32-unknown-unknown` std takes it from a port of musl's libm, natively from the platform's
libm. A wasm32 build of geo, run in wasmi beside its native build:

- `hypot` gave a different last bit for 2,032 of 2,000,000 random inputs across 40 binary orders
  of magnitude, and for 12,628 of 2,000,000 inputs within ±500;
- `Simplify` on the case's three lines, at 8 tolerances in map units and in pixels, gave identical
  bits on both.

A point is kept on one target and dropped on the other only where its distance lies within one
last bit of the tolerance or of an equally far point. geo's buffer with round joins takes `sin`,
`cos`, and `acos` from `libm` through i_float 1.16's `FloatNumber` for `f64`. Its miter join calls
std's `cos` and `tan`, which a round join does not reach. The workspace's `clippy.toml` bans these
functions in Forager's own code. It does not reach geo's.

**The crates' cost.** geo 0.33.1 with its default features off builds for
`wasm32-unknown-unknown`, links no C, and passes `cargo deny check` with the workspace's
`deny.toml`. It adds 19 crates to `Cargo.lock`:

- i_overlay 4.5.2 with i_float 1.16, i_shape 1.18, i_key_sort 0.10, and i_tree 0.18, beside the
  9.1.0 family of [ADR-0028](0028-hatch-cut-by-i-overlay.md);
- `rand` 0.10 with `rand_core` and `rand_pcg`, which geo 0.33.1 calls only in its k-means, off the
  output path (Q-2);
- geo itself, `geo-types`, `rstar`, `robust`, `geographiclib-rs`, `sif-itree`, `float_next_after`,
  `approx`, `heapless`, `hash32`, and `byteorder`.

Growth of the browser bindings' gzipped `.wasm`, 997,445 bytes at faff23d3, with each probe called
from the layer's transform:

| Probe | Growth |
|---|---|
| geo `Simplify` | +5,332 B |
| `Simplify` and `unary_union` | +72,694 B |
| `Simplify`, `unary_union`, and `Buffer` | +82,378 B |
| `Simplify`, `Validation`, `unary_union`, and `Buffer` | +129,443 B |

**The maintainer's choice.** On 2026-11-15 the maintainer chose the geo crate for the geometry
code, against the session's recommendation of a port of JTS. The maintainer chose geo's
`Validation` for the validity test and geo's `Simplify` over a Douglas-Peucker of Forager's own.
`simplifypt` stays not drawn until a later decision.

**The maintainer's reversal.** The implementation found that geo's `Simplify` recurses once for
each point it keeps. A line that zigzags with a decreasing amplitude keeps every point:

- natively, in a thread of 1 MiB, 4,000 points ran and 4,500 overflowed the stack, about 220 bytes
  a level;
- a wasm32 build of geo 0.33.1 run in wasmi 2.0.0, the engine of the G-9 test, ran 900 points and
  trapped with a stack overflow at 1,000.

The bound of Q-9 could not keep such a line out, since a part of about 14,000 points passes it
alone. On 2026-11-15 the maintainer chose a Douglas-Peucker of Forager's own, ported from JTS's
`DouglasPeuckerLineSimplifier`, with a stack of sections of its own in place of recursion. It also
removes geo's `hypot` from the path of `simplify` and the difference of the rule for equally far
points. geo stays for `Validation`, `unary_union`, and `buffer`. The maintainer kept `unary_union`
for the repair, with its measured difference stated.

**Forces.**

- Q-4 and G-6: the cases pass by a raster metric.
- Q-1: no C. Q-6: permissive licences, and no code of GEOS in the tree.
- Q-2 and G-9: the same bits on every target.
- Q-5: the bundle stays under 3 MB, about 2 MB above today's size.
- Q-9: a buffer and a repair add points, and a simplification visits them.
- Q-3: no input may crash the render, the browser's included.
- [ADR-0008](0008-ports-and-adapters-crates.md): a crate of the workspace needs a row of its
  table; a crate from crates.io needs none.
- [ADR-0028](0028-hatch-cut-by-i-overlay.md): i_overlay must be a dependency of `render-agg`
  alone. The test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test` holds it on
  direct dependencies. geo brings its own i_overlay into `symbolize`, which that clause forbids.

## Decision

We will compute `simplify` in a geometry module of `crates/symbolize` by a Douglas-Peucker ported
from JTS. We will compute `buffer`, and a simplified polygon's validity and repair, with geo.

- **Place.** The module lives in `crates/symbolize`, beside the layer's transform. A layer's
  transform and a style's expression call the same functions. No crate of the workspace is added.
- **Dependency.** geo must be a direct dependency of `symbolize` alone among the workspace's
  crates, at 0.33, with its default features off.
- **Precondition.** geo brings i_overlay 4.5 into `symbolize`, which ADR-0028 forbids. This ADR
  must not be accepted before
  [ADR-draft-i-overlay-in-render-agg-and-geo](draft-i-overlay-in-render-agg-and-geo.md), the
  successor of ADR-0028 that allows it.
- **Douglas-Peucker.** Forager's own Douglas-Peucker must port JTS's
  `DouglasPeuckerLineSimplifier`:
  - it must split a section at the first of its farthest points, as JTS's `>` decides;
  - it must compute each distance as JTS's `Distance.pointToSegment`, by `f64::sqrt`;
  - it must keep its pending sections on a stack of its own and must not recurse.
- **`simplify` of a line** must be that Douglas-Peucker on each part.
- **`simplify` of a polygon** must take these steps, in this order:
  - each ring is simplified by that Douglas-Peucker as a line;
  - a ring of 4 or more coordinates loses its first point as JTS's
    `DouglasPeuckerLineSimplifier` decides, with the distance computed by `f64::sqrt`;
  - a ring with fewer than 4 coordinates is removed. A removed shell leaves an empty polygon;
  - geo's `Validation` decides the result's validity;
  - an invalid result is repaired by geo's `unary_union`. A multipolygon is repaired as a whole. A
    valid result is not passed through `unary_union`.
- **`buffer`** must be geo's `buffer_with_style` with round joins and round caps of π/60 radians,
  which are 30 segments a quadrant.
- **Reference.** Forager's own steps must follow JTS's sources, not GEOS's, which are under
  LGPL-2.1. Each file that ports a JTS source must name it and hold JTS's EDL notice.
  `THIRD-PARTY-NOTICES.md` must hold that notice too.
- **Mathematics.** Forager's own steps must take every transcendental function from `libm`. They
  may use `f64::sqrt` and IEEE's arithmetic.
- **Bounds.** The points a function adds and visits must count against the budget of Q-9 that
  `crates/symbolize/src/layer_transform.rs` keeps for a layer's transform.
  - The Douglas-Peucker must charge the points of each section it scans before it scans them.
  - The points a repair returns beyond those it was given must count as points added.

**Out of scope:**

- `simplifypt`. geo's `SimplifyVwPreserve` does not reproduce GEOS, so `simplifypt` stays a
  `not_drawn` reason. The backlog entry `docs/backlog/simplifypt-topology-preserving.md` names the
  options;
- the transform tree's nodes and their arguments, which
  [ADR-draft-transform-tree-with-simplify-and-buffer](draft-transform-tree-with-simplify-and-buffer.md)
  decides;
- a tolerance of 0 or below, for which geo returns its input and GEOS does not, and MapServer's
  reaction when GEOS fails. `docs/semantics/` writes them down from the C, and the module follows
  that page;
- whether the engine draws a style's expression, which the fidelity plan's ticket of style
  expressions decides;
- `centerline`, `difference`, and other functions of GEOS that no case in scope draws.

## Alternatives considered

- **A port of JTS 1.20.0 for all three functions, with i_overlay 9.1.0's union** — the session's
  recommendation. It follows GEOS's rule for equally far points and computes distances with
  `f64::sqrt`, so the differences and the risk to G-9 below vanish. It adds no crate, and it brings
  `simplifypt` within reach. Not chosen by the maintainer: it is the largest piece of geometry
  Forager would own, about 2,000 lines of Java to port.
- **A partial port of JTS's `IsValidOp` for the validity test** — only the checks a simplification
  can break, and no 47 KB of geo's `Validation` in the bundle. Not chosen by the maintainer: it is
  code of Forager's own, where geo's check agreed with GEOS on every result measured.
- **geo's `Simplify` for lines and rings** — no Douglas-Peucker of Forager's own, the
  maintainer's first choice. It recurses once for each point it keeps, and overflowed the stack at
  1,000 points in wasmi and at 4,500 natively in a thread of 1 MiB (*Context*). It keeps the last
  of equally far points, where GEOS keeps the first: 0.7 % to 2.8 % of polygons on an integer grid
  differ. Its distance calls `hypot`. Not chosen after the maintainer's reversal.
- **A bound on the points of a simplified part** — geo's `Simplify` kept, a part past about 900
  points refused. A new limit of Q-9, a change to the specification, and it refuses real data.
- **geo's `Polygon::simplify` as it stands** — no code of Forager's own. It keeps collapsed rings
  and the ring's first point, and repairs nothing (*Context*).
- **geo's `SimplifyVwPreserve` for `simplifypt`** — Visvalingam-Whyatt, whose tolerance is an area.
  It differs from GEOS on 4 of the case's 6 line results.
- **i_overlay 9.1.0's `OutlineOffset` for `buffer`** — the crate already in the tree, +42,772 B.
  Every vertex within 0.027 px of GEOS's, but 4 to 7 more points per ring.
- **A port of GEOS's C++** — rejected: LGPL-2.1, against Q-6 and the *Constraints*.
- **The `geos` crate** — rejected: it binds GEOS's C library through `geos-sys`, against Q-1.
- **`geo-buffer` 0.2.0** — rejected: it buffers by a straight skeleton, with mitred corners.
- **A crate of the workspace of its own, such as `geometry`** — one consumer today. A crate would
  need a row in ADR-0008's table.

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapgeos.c` (`:899`, `:922`, `:946`) and `src/mapparser.y` (`:758`,
  `:842`, `:856`), read 2026-11-15 from `target/ms-src`.
- GEOS 3.12.1, release tarball from download.osgeo.org, its `NEWS.md` (GH-773, GH-784, GH-986),
  built and called 2026-11-15; GEOS 3.13.1 from Debian trixie. Their output decided the
  measurements. Their sources were read to confirm when GEOS repairs and removes a ring; they are
  not a reference for Forager's code.
- JTS, github.com/locationtech/jts: `LICENSES.md` (EPL-2.0 or EDL-1.0), the EDL at
  eclipse.org/org/documents/edl-v10.php (SPDX `BSD-3-Clause`), `DouglasPeuckerSimplifier.java` and
  `DouglasPeuckerLineSimplifier.java` on `master`, read 2026-11-15.
- geo 0.33.1 and geo-types 0.7.20, crates.io: `algorithm/simplify.rs`, `algorithm/buffer.rs`,
  `algorithm/bool_ops/mod.rs` (`unary_union`), `algorithm/validation/`, `private_utils.rs`
  (`line_segment_distance`); i_overlay 4.5.2 `mesh/` and i_float 1.16 `float/number.rs`.
- packages.ubuntu.com, noble, `libgeos-dev` 3.12.1-3build1, read 2026-11-15.
- [ADR-0008](0008-ports-and-adapters-crates.md), [ADR-0011](0011-crate-supply-chain.md),
  [ADR-0028](0028-hatch-cut-by-i-overlay.md), [ADR-0034](0034-wasm-renders-compared-in-wasmi.md),
  [ADR-0037](0037-reprojection-ported-in-proj.md).

## Consequences

- Positive: the case of `vertex2` reproduces GEOS's points bit for bit, on both targets as
  measured. A polygon's simplification matched GEOS on every polygon measured with 6 decimals.
  geo's `Validation` agreed with GEOS's `isValid` on all 3,053 results. `buffer` comes within
  2.5 × 10⁻⁷ px on the case's polygons. The Douglas-Peucker keeps GEOS's rule for equally far
  points and needs no recursion, so no line overflows the stack, and `hypot` is off its path.
  The budget is charged by the work done, so a long part is simplified as MapServer simplifies it.
- Negative / trade-offs, measured:
  - a buffer and a repaired polygon are not bit-identical to GEOS. A buffer of −3 px differed by up
    to 2.1 × 10⁻³ px, with 2 of 272 rings of another point count;
  - the repair by `unary_union` keeps another lobe than GEOS's buffer of 0 for 7 of 40 repaired
    random polygons, where the simplified ring crosses itself;
  - Forager owns a Douglas-Peucker of about 60 lines besides the first-point step and the removal
    of a ring;
  - 19 crates, among them a second i_overlay family and `rand`, and +129 KB of the bundle with
    `buffer` and `Validation`, of which `Validation` takes about 47 KB.
- Negative / trade-offs: `simplifypt` stays not drawn, so the style `simplifypt([shape],50)` of
  `geomtransform-vertex.map` draws nothing.
- Follow-ups:
  - the backlog entry of `simplifypt`;
  - a measurement, once `buffer` is drawn, of whether a buffer of 0 repairs closer to GEOS than
    `unary_union`;
  - a record of GEOS's results made in the noise run, as
    [ADR-0038](0038-proj-references-from-mapserver.md) records PROJ's, in place of coordinates
    taken by hand;
  - the semantics page of the functions, a tolerance of 0 and GEOS's failures among them.

## Enforcement

- Unit tests of the geometry module cite this ADR with `Verifies:`. They hold coordinates taken
  from GEOS 3.12.1:
  - the case's three lines at 25 map units and at 10 px, bit for bit;
  - a polygon whose ring loses its first point, one with a removed hole, and one with a removed
    shell, bit for bit;
  - a polygon whose simplified result geo's `Validation` finds invalid, repaired with GEOS's point
    count, each vertex within 10⁻⁶ of GEOS's in the shape's unit;
  - a polygon whose simplified result is valid, left as it is, bit for bit;
  - a line and a polygon on an integer grid whose equally far points GEOS splits at the first, bit
    for bit;
  - `world_testpoly.shp` in the pixels of `polygon-as-line-offset.map`, buffered by 1 and by −1,
    with GEOS's point count, each vertex within 10⁻⁶ px of GEOS's.
- The msautotest harness renders `renderers/geomtransform-vertex.map` with `-l vertex2` natively
  and through the wasm32 module in wasmi, and compares the buffers bit for bit (G-9,
  [ADR-0034](0034-wasm-renders-compared-in-wasmi.md)).
- A test renders a layer of `simplify` over a zigzag line of several thousand points, each kept,
  natively and through the wasm32 module in wasmi. Both must render without a trap, bit for bit
  alike (Q-3, G-9).
- A unit test holds the Douglas-Peucker's charge to the sections it scans (Q-9).
- The test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test` holds geo to
  `symbolize`, as the successor of ADR-0028 decides. It cites this ADR with `Verifies:` too.
- When the engine draws `buffer`, the harness renders `renderers/polygon-as-line-offset.map`
  natively and through the wasm32 module, and compares the buffers bit for bit (G-9).
- `scripts/check-notice.sh` fails on a file under `crates/symbolize` that names a JTS source without
  JTS's notice.
- `cargo deny check` runs on every change to `Cargo.lock` ([ADR-0011](0011-crate-supply-chain.md)).

**Not mechanically decidable:** that geo's `Validation`, `unary_union`, and `buffer` give the same
bits on every target for an input no test holds. The bit-identity runs cover the cases only.
