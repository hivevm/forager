# ADR-draft-geometry-ported-from-jts: `simplify`, `simplifypt`, and `buffer` are computed in `symbolize` by a port of JTS, with i_overlay's union where JTS builds a planar graph

- **Status:** 🟡 proposed
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the geometry module of `crates/symbolize` that computes `simplify`, `simplifypt`, and `buffer` and its callers in `crates/symbolize/src/layer_transform.rs`, the `[dependencies]` of `crates/symbolize`, `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of JTS, and the unit tests that hold coordinates taken from GEOS

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

**Measurements** (2026-11-15, in `.scratch/`, outside the workspace). GEOS 3.12.1 was built from
its release tarball, GEOS 3.13.1 taken from Debian. A C program called the three GEOS functions as
`msGEOSShape2Geometry` builds the geometry. A Rust program called geo 0.33.1 and i_overlay 9.1.0 on
the same coordinates. Both printed every coordinate with 17 significant digits.

| Input | geo 0.33.1 against GEOS 3.12.1 |
|---|---|
| `testlines.shp`, `Simplify` at 1, 5, 10, 25, 50 map units | bit-identical, all 3 lines |
| the same in the pixels of `geomtransform-vertex.map`, at 10 and 50 | bit-identical |
| `testlines.shp` in pixels, GEOS `TopologyPreserveSimplify` at 10 and 50 against geo `Simplify` | bit-identical |
| the same against geo `SimplifyVwPreserve` at 10 and 50 | 4 of 6 lines differ: 4 points against 9, 5 against 7, 3 against 8, 2 against 3 |
| `world_testpoly.shp`, `Simplify` at 1, 3, 5 units and at 2, 5 pixels | bit-identical, all 4 polygons |
| the same at 10, 20, 40 | 1 to 4 of 4 differ: GEOS drops a collapsed hole, returns `POLYGON EMPTY` for a collapsed shell, and repairs a self-intersecting result, while geo keeps all three |
| `world_testpoly.shp`, `TopologyPreserveSimplify` at 10, 20, 40 against geo `Simplify` | 1 to 3 of 4 differ: GEOS removes the ring's first point, geo keeps it |
| `world_testpoly.shp` in pixels, `buffer` ±1, geo's default style | 37 to 51 points against GEOS's 123 to 177, every vertex within 0.104 px |
| the same, geo with round joins and caps of π/60 | the same number of points, every vertex within 2.5 × 10⁻⁷ px, no coordinate identical |
| the same, i_overlay 9.1.0's `OutlineOffset` directly, round joins of π/60 | 4 to 7 more points per ring, every vertex within 0.027 px; 0.014 px with its float mode |

GEOS 3.12.1 and 3.13.1 gave identical results but one: a repaired polygon of `Simplify` at 20,
whose computed vertex differs in its last bit. A repair so moves with the GEOS release.

GEOS's Douglas-Peucker differs from geo's `Simplify` in four details the table shows or the JTS
sources state. It keeps the first of two equally far points, where geo keeps the last. It may drop
a ring's first point (GEOS 3.12.0, GH-773 and GH-784). It removes a ring that collapses. It repairs
an area that its simplification made invalid, by a buffer of 0. geo's `SimplifyVwPreserve` is not
GEOS's topology-preserving simplifier: it is Visvalingam-Whyatt, and its tolerance is an area, not a
distance.

**The crates' cost**, measured on the browser bindings built by `scripts/build-web.sh` at faff23d3,
whose gzipped `.wasm` is 997,445 bytes. Each probe called the code from the layer's transform so
that the optimizer kept it.

| Probe | Growth, gzipped |
|---|---|
| geo `Simplify` | +5,332 B |
| geo `Simplify` and `SimplifyVwPreserve` | +32,304 B |
| geo `Simplify`, `SimplifyVwPreserve`, and `Buffer` | +113,150 B |
| i_overlay 9.1.0 float union, already a dependency of `render-agg` | +30,773 B |
| i_overlay 9.1.0 `OutlineOffset` | +42,772 B |

geo 0.33.1 with its default features off builds for `wasm32-unknown-unknown`, links no C, and passes
`cargo deny check` with the workspace's `deny.toml`. It adds 19 crates to `Cargo.lock`. Among them
are i_overlay 4.5.2 with its own i_float, i_shape, i_key_sort, and i_tree, beside the 9.1.0
family of [ADR-0028](0028-hatch-cut-by-i-overlay.md), and `rand` with `rand_pcg`. Its distance calls
`f64::hypot`, which the workspace's `clippy.toml` bans on the output path (Q-2,
[ADR-0037](0037-reprojection-ported-in-proj.md)).

**Forces.**

- Q-4 and G-6: the cases pass by a raster metric.
- The ticket's criteria ask more: GEOS's points for a line and a polygon.
- Q-1: no C.
- Q-2 and G-9: the same bits on every target, so no platform libm on the output path.
- Q-5: the bundle stays under 3 MB, with about 2 MB of headroom today.
- Q-6 and the *Constraints*: permissive licences only. No GEOS code may enter the tree.
- Q-9: a buffer adds points, and a simplification visits them. The layer's budget bounds both.
- [ADR-0008](0008-ports-and-adapters-crates.md): a new crate needs a new row of its table.
- [ADR-0028](0028-hatch-cut-by-i-overlay.md): i_overlay must be a dependency of `render-agg`
  alone. Its *Out of scope* leaves a `buffer` free to choose i_overlay by an ADR of its own.
- Simplicity: no dependency without a present need.

## Decision

We will compute `simplify`, `simplifypt`, and `buffer` in a geometry module of `crates/symbolize`,
by our own port of JTS 1.20.0, with i_overlay's union where JTS builds a planar graph.

- **Place.** The module lives in `crates/symbolize`, beside the layer's transform. A layer's
  transform and a style's expression call the same functions. No crate is added.
- **`simplify`** must port JTS's `DouglasPeuckerSimplifier` and `DouglasPeuckerLineSimplifier`:
  - the farthest point is the first of equally far points;
  - a ring may lose its first point;
  - a ring that collapses is removed, and a collapsed shell leaves an empty polygon;
  - an area whose simplification is invalid is repaired, as GEOS repairs it by a buffer of 0.
- **`simplifypt`** must port JTS's `TopologyPreservingSimplifier` with its tagged line strings and
  their segment index.
- **`buffer`** must port JTS's raw buffer curve: `BufferCurveSetBuilder`, `OffsetCurveBuilder`,
  `OffsetSegmentGenerator`, and `BufferInputLineSimplifier`, with 30 segments a quadrant and round
  caps and joins.
- **i_overlay.** i_overlay's union must resolve the raw buffer curves into polygons, and must repair
  an invalid simplified area. It stands in for JTS's noding, planar graph, and polygon builder,
  which are not ported.
- **Validity.** The module must decide an area's validity as JTS's `IsValidOp` decides it. It must
  check the faults a simplification can make:
  - a ring that crosses itself or another ring;
  - a hole outside its shell.

  A valid area must not pass through i_overlay.
- **Reference.** The port must follow JTS 1.20.0's sources. It must not follow GEOS's sources,
  which are under LGPL-2.1. Each file that ports a JTS source must name it and hold JTS's EDL
  notice. `THIRD-PARTY-NOTICES.md` must hold that notice too.
- **Fidelity.** Unit tests must hold coordinates taken from GEOS 3.12.1:
  - a result of `simplify` or `simplifypt` that GEOS does not repair must equal GEOS's
    coordinates bit for bit;
  - a repaired result and a buffer must have GEOS's rings;
  - each vertex of those rings must lie within a tolerance of GEOS's vertex. The tolerance is 10⁻⁶
    times the larger of 1 and the extent of the shape's bounding box, in the shape's unit.
- **Mathematics.** The module must take `sin`, `cos`, `atan2`, and every other transcendental
  function from `libm`. It may use `f64::sqrt` and IEEE's arithmetic.
- **Bounds.** The points a function adds and visits must count against the layer's budget of Q-9,
  as the ported functions of [ADR-0036](0036-typed-geomtransform.md) count them.

**Out of scope:**

- the transform tree's nodes and their arguments, which the successor of
  [ADR-0036](0036-typed-geomtransform.md) decides;
- how MapServer reacts when GEOS fails, such as for a negative tolerance of `simplify`, which
  `docs/semantics/` writes down from the C;
- a style's expression being drawn at all, which the fidelity plan's ticket of style expressions
  decides;
- `centerline`, `difference`, and other functions of GEOS that no case in scope draws;
- the change to ADR-0028's dependency clause that this decision needs (*Consequences*).

## Alternatives considered

- **The geo crate, 0.33.1.** Its `Simplify` reproduces GEOS bit for bit on every line of the case
  and on every polygon GEOS neither collapses nor repairs. It differs on the rest (*Context*).
  Its `SimplifyVwPreserve` is another algorithm with an area as its tolerance, so `simplifypt`
  would still need a port of JTS's topology-preserving simplifier. Its buffer with round joins of
  π/60 comes within 2.5 × 10⁻⁷ px of GEOS. Not chosen: it adds 19 crates and 113 KB gzipped for all
  three functions. It brings a second i_overlay family and `rand`. It calls the platform's `hypot`,
  which Q-2 bans on the output path. It meets neither criterion for polygons.
- **geo for `simplify` and `buffer`, a JTS port for `simplifypt`.** The case of `vertex2` passes
  with the least code. Not chosen: the port of JTS's line simplifier is the core of the topology
  port anyway. geo's polygon details would still miss GEOS. The dependency cost stays.
- **i_overlay 9.1.0's `OutlineOffset` for `buffer`.** No port, and +43 KB. Every vertex lies
  within 0.027 px of GEOS's, but the rings carry 4 to 7 more points. It is the fallback if the
  port of the raw curve proves too costly. Then the tolerance of the *Fidelity* entry for a buffer
  must be 0.03 px.
- **A full port of JTS's buffer, its noding, planar graph, and polygon builder included.** The
  closest to GEOS, likely bit-identical. Not chosen: it is several thousand lines of Java for two
  styles of one case. i_overlay already does the union.
- **A port of GEOS's C++.** Rejected: GEOS is under LGPL-2.1, against Q-6 and the *Constraints*.
  A clean-room port is possible for Douglas-Peucker itself, which is published (Douglas and Peucker,
  1973). GEOS's details beyond it, the ring's first point, the collapse, and the repair, come from
  JTS. JTS offers them under a BSD licence, so no clean room is needed.
- **The `geos` crate.** Rejected: it binds GEOS's C library through `geos-sys`, against Q-1 and
  ADR-0008's ban of C.
- **`geo-buffer` 0.2.0.** Rejected: it buffers by a straight skeleton, so its corners are mitred,
  not GEOS's round joins.
- **A crate of its own, such as `geometry`.** One consumer, `symbolize`, needs it today. A crate
  would need a row in ADR-0008's table, so a successor of ADR-0008. It may move there when
  `label` or `render` needs it.

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapgeos.c` (`msGEOSBuffer` `:899`, `msGEOSSimplify` `:922`,
  `msGEOSTopologyPreservingSimplify` `:946`) and `src/mapparser.y` (`:758`, `:842`, `:856`), read
  2026-11-15 from `target/ms-src`.
- GEOS 3.12.1 release tarball, download.osgeo.org, its `NEWS.md` (GH-773, GH-784, GH-986), built
  and called 2026-11-15. GEOS 3.13.1 from Debian trixie. Only their output was compared. Their
  sources are not a reference for the port.
- JTS, github.com/locationtech/jts: `LICENSES.md` (EPL-2.0 or EDL-1.0), the EDL at
  eclipse.org/org/documents/edl-v10.php (SPDX `BSD-3-Clause`), the sources of
  `org.locationtech.jts.simplify` on `master`, read 2026-11-15, and the release 1.20.0 (2024-08-30).
- geo 0.33.1, crates.io: `src/algorithm/simplify.rs`, `simplify_vw.rs`, `buffer.rs`.
- i_overlay 9.1.0, crates.io: `src/mesh/float/outline/offset.rs`, `src/mesh/float/style.rs`.
- packages.ubuntu.com, noble, `libgeos-dev` 3.12.1-3build1, read 2026-11-15.
- D. H. Douglas and T. K. Peucker, "Algorithms for the reduction of the number of points required
  to represent a digitized line or its caricature", *The Canadian Cartographer* 10(2), 1973.
- [ADR-0008](0008-ports-and-adapters-crates.md), [ADR-0011](0011-crate-supply-chain.md),
  [ADR-0028](0028-hatch-cut-by-i-overlay.md), [ADR-0036](0036-typed-geomtransform.md),
  [ADR-0037](0037-reprojection-ported-in-proj.md), whose port of PROJ this decision follows.

## Consequences

- Positive: no crate is added. The bundle grows by the port and by i_overlay's float union, which
  is about 31 KB. The port follows GEOS's details, so `simplify` and `simplifypt` meet the ticket's
  bit-for-bit criterion where GEOS repairs nothing. `libm` keeps G-9.
- Positive: the code is the reference's own algorithm under a permissive licence, readable beside
  JTS. A later function of GEOS, `centerline` or `difference`, follows the same path.
- Negative / trade-offs: the port is the largest piece of geometry Forager owns. The topology
  simplifier and the raw buffer curve are about 2,000 lines of Java. The validity test is new code
  of Forager's own. JTS 1.20.0 is a year younger than GEOS 3.12.1, so a test against GEOS may
  find a detail where the two differ. Then the test against GEOS decides, and the port names the
  detail.
- Negative / trade-offs: a repaired area and a buffer are not bit-identical to GEOS. i_overlay snaps
  to its integer grid where GEOS computes intersections in doubles. GEOS's own releases already
  differ there in the last bit.
- Follow-ups:
  - a successor of ADR-0028, whose *Decision* makes i_overlay a dependency of `render-agg` alone;
    i_overlay must become a dependency of `symbolize` too;
  - a record of GEOS's results made in the noise run, as
    [ADR-0038](0038-proj-references-from-mapserver.md) records PROJ's, in place of coordinates
    taken by hand;
  - the semantics page of the three functions, GEOS's failures among them.

## Enforcement

- Unit tests of the geometry module cite this ADR with `Verifies:`. They hold coordinates taken
  from GEOS 3.12.1 for a line and a polygon of each function. They cover:
  - equally far points;
  - a ring's first point removed;
  - a collapsed hole and a collapsed shell;
  - a repaired area, within the tolerance of the *Fidelity* entry;
  - a buffer by a positive and a negative distance, within that tolerance.
- The msautotest harness renders `renderers/geomtransform-vertex.map` with `-l vertex2`, natively
  and through the wasm32 module, and compares the buffers (G-9).
- `crates/symbolize/clippy.toml` bans the platform's transcendental functions, as it does today.
- `scripts/check-notice.sh` fails on a file under `crates/symbolize` that names a JTS source without
  JTS's notice.
- `cargo deny check` runs on every change to `Cargo.lock` ([ADR-0011](0011-crate-supply-chain.md)).

**Not mechanically decidable:** that the port follows JTS and not GEOS. Review compares a ported
file with the JTS source it names.
