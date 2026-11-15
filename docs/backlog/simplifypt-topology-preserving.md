# `simplifypt` is not drawn: geo has no simplifier that reproduces GEOS's

**Observed:** the research of fidelity ticket 11 (2026-11-15). MapServer computes `simplifypt`
through `GEOSTopologyPreserveSimplify_r` (`src/mapgeos.c:946`), GEOS's
`TopologyPreservingSimplifier`, a Douglas-Peucker that refuses a simplification which would make
the result cross itself or another ring. The maintainer chose the geo crate for the geometry code
(ADR-0041). geo's `SimplifyVwPreserve` is Visvalingam-Whyatt, whose
tolerance is an area, not a distance. Against GEOS 3.12.1 it kept 9, 7, 8, and 3 points where GEOS
kept 4, 5, 3, and 2, on 4 of the 6 results of `testlines.shp` at 10 and 50 px. So `simplifypt`
stays a `not_drawn` reason of the transform tree.

**Where:** the style `simplifypt([shape],50)` of msautotest's `renderers/geomtransform-vertex.map`
(a style's expression, fidelity ticket 10); the `not_drawn` reason `simplifypt` in
`crates/style-ir/src/transform.rs`.

**What a change weighs:**

- **A port of JTS's `TopologyPreservingSimplifier` alone** (EDL-1.0, BSD-3-Clause): its tagged line
  strings, their segment index, and the ring's first point (GEOS GH-784, GH-986). GEOS's output
  is the reference; on the case's lines at 10 and 50 px, plain Douglas-Peucker already equals it,
  but on `world_testpoly.shp` at 10 to 40 it differs on 1 to 3 of 4 polygons. A port is several
  hundred lines of Java with JTS's notice, and needs a successor of the ADR of the transform tree
  to add the node.
- **Accepting geo's `SimplifyVwPreserve`**: no code, but another algorithm and another meaning of
  the tolerance, so the node would not mean what MapServer's function means.
- **Leaving it not drawn**: the case's style draws nothing, as today.
