# ADR-draft-map-rotation-in-the-reprojection: A map's `ANGLE` is MapServer's geotransform, a step of every layer's reprojection in `proj`, and a rotated map is drawn in the frame of its image

- **Status:** 🟡 proposed
- **Date:** 2026-11-16
- **Deciders:** Markus Brigl
- **Applies to:** the geotransform of a rotated map in `crates/proj` and its use by the reprojection of a layer's features and of the area asked of its source (`crates/proj/src/rect.rs`, `crates/proj/src/shape.rs`, `crates/engine/src/reproject.rs`), the view, the scale, and the scale factors of a rotated map in `crates/engine` and the `View` of `crates/symbolize/src/extent.rs`

## Context

A mapfile's `MAP` may hold `ANGLE`, a number from -360 to 360 (`loadMapInternal` in `src/mapfile.c`
at the pin, line 6733). Three msautotest cases in scope draw a rotated map: `misc/rot_out.map`
(`ANGLE 30`, map and layer in `+proj=utm +zone=11 +datum=WGS84`), `renderers/symbolrot.map` (`ANGLE
45`) and `renderers/symbolrot_negative.map` (`ANGLE -315`), both with map and layers in
`init=epsg:21781`. Every one of them names the map's projection again on each layer.

**How MapServer rotates a map, at the pin.**

1. `msMapSetRotation` (`src/mapobject.c:350`) keeps the angle and sets `gt.need_geotransform` where
   it is not 0.
2. `msPrepareImage` (`src/mapdraw.c:132`) grows the extent by `msAdjustExtent`, computes the scale
   from that grown extent, and then calls `msMapComputeGeotransform` (`:236`). With
   `res_x = (maxx - minx) / (width - 1)`, `res_y` alike, `θ = angle * MS_PI / 180`, and the
   extent's centre `(cx, cy)`, `msMapComputeGeotransformEx` (`src/mapobject.c:381`) sets
   `gt = [cx - w/2·gt1 - h/2·gt2, cos θ·res_x, sin θ·res_y, cy - w/2·gt4 - h/2·gt5, sin θ·res_x,
   -cos θ·res_y]` and its inverse by `InvGeoTransform` (`src/mapresample.c:45`). The image's
   width and height, not width - 1 and height - 1, centre it (bug 1916's comment).
3. `msMapSetFakedExtent` (`src/mapobject.c:465`, called at `src/mapdraw.c:240`) replaces the map's
   extent by `0, 0, width, height` and its cellsize by 1. It copies the geotransform onto the
   map's projection, flipped so that y runs up, and marks every layer to reproject.
4. `msProjectionsDiffer` (`src/mapproject.c:2149`) returns false where either projection has no
   parameters, then true where their numbers of arguments differ, then true where either holds a
   geotransform. So on a rotated map every layer with a
   projection is reprojected, the map's own projection among them (`src/mapdraw.c:848`).
5. `msProjectPointEx` (`src/mapproject.c:876`) applies the input side's geotransform, then PROJ,
   then the output side's inverse geotransform. A feature goes from the layer into the map's
   faked extent: PROJ, then the inverse geotransform. The area asked goes the other way.
   `msDrawVectorLayer` (`src/mapdraw.c:1144-1174`) projects the faked extent by `msProjectRect`
   only where map and layer both have parameters. It samples the faked rectangle, applies the
   geotransform to each sample, then PROJ.
6. Where both projections are one and the same argument, as `init=epsg:21781` is,
   `msProjectCreateReprojector` (`src/mapproject.c:422`) creates no PROJ object, and only the
   geotransform moves a point. Where they are the same several arguments, as rot_out's `+proj=utm
   +zone=11 +datum=WGS84`, it creates a PROJ object from one CRS to the same CRS.
   `msProjectRect` adds `+over` to each side whose first argument starts with `init=` or `proj=`
   (`:2072-2121`). So the area asked of the cases in scope always goes through such a PROJ object.
7. Everything after the reprojection works in the faked extent. That covers the transform to
   pixels (x as it is, y as height - y), the clip, `GEOMTRANSFORM`, and the label cache, which
   `msDrawMap` draws before it restores the real extent (`src/mapdraw.c:593`, `:660`).
8. The scale factor of a size in units other than pixels comes from `msGetGeoCellSize`
   (`src/mapdraw.c:43`). It is `sqrt(cx² + cy²) / sqrt(2)` of the real extent over width and over
   height, not the cellsize.
9. The tests of the reprojection that read the geotransform read it as MapServer holds it. With a
   geotransform on the map, `msProjectRect` skips its polar branches (`:1981`, `:2015`), and
   `msProjectGetLineCuttingCase` reads it (`:1062-1113`). `msProjectShapeLine` measures the
   dateline wrap through it only where `gt[2]` is 0 (`:1406-1418`), which a rotation other than a
   multiple of 180° never gives.

**The numbers of `misc/rot_out.map`**, from the C's formulas, in IEEE doubles:

- grown extent `478153.47826086957, 4762880, 481796.52173913043, 4765610`, cellsize
  `9.130434782608695`, scale denominator `25881.56139130431`;
- the map's geotransform `[477708.7796974371, 7.907188469336169, 4.565217391304347,
  4764518.03479214, 4.565217391304341, -7.907188469336179]`;
- the flipped geotransform of the map's projection `[479078.34491482837, 7.907188469336169,
  -4.565217391304347, 4762145.878251339, 4.565217391304341, 7.907188469336179]`;
- the area asked, with PROJ's step the identity, `477708.7796974371, 4762145.878251339,
  482241.2203025628, 4766344.121748662`, the bounding box of the rotated view's four corners;
- `msGetGeoCellSize` gives `9.10380514271694`, where the cellsize is `9.130434782608695`;
- the extent's centre lands on pixel `(200, 150.00000000017)`, about `(200, 150)`, where an
  unrotated map puts it on `(199.5, 149.5)`.

**Where this sits against the decisions in force.**
[ADR-0037](0037-reprojection-ported-in-proj.md) decides how `proj` reprojects: its own port of
PROJ's formulas, `libm`, and no other dependency. It names the reprojection of a layer's features
and of the area asked in `crates/engine`. It leaves out of its scope the ports of MapServer's code
that live in `proj`: the area asked as `msProjectRect` computes it (`rect.rs`), and horizon
clipping and the dateline of `+over` (`shape.rs`, which ports `msProjectShapeLine`).
[ADR-0038](0038-proj-references-from-mapserver.md) measures them. The rotation changes neither
PROJ's formulas nor the mathematics. It changes what the reprojection does in three ways that
ADR-0037 does not name:

- a step that is MapServer's, not PROJ's, inside `rect.rs` and `shape.rs`, before the formulas for
  the area asked and after them for features;
- a path for a layer in the map's projection, which today is not reprojected at all
  (`reproject::for_layer` returns none where the strings do not `differ`);
- a frame after the reprojection that is the image's, not the map's coordinates.

[ADR-0039](0039-declared-projection.md) is not changed. A layer of `AUTO` whose source declares a
projection, or none, has parameters and is reprojected as any other. A map of `AUTO` with a layer
of `AUTO` holds the one argument `AUTO` on both sides, and MapServer applies only the
geotransform there (*Context*, 6). A source that declares
`NoProjection` leaves the layer without parameters (see *Without a projection* below).

The style IR has no field for the map's angle. The map's angle enters it as an optional field, an
addition [ADR-0014](0014-style-document-versions.md)'s version rule allows. The maintainer keeps
the style document at format version 1.1 and regenerates the schema of 1.1 for it. That is
ADR-0014's to govern, not this decision's.

## Decision

We will make a map's geotransform a step of each layer's reprojection in `proj`, with the image as
the frame after it.

- **The geotransform.** `proj` must compute it as `msMapComputeGeotransformEx` computes it from the
  grown extent, the image's size, and the angle. `proj` must flip it as `msMapSetFakedExtent`
  does, and invert it as `InvGeoTransform` does. `proj` must take the angle in radians as
  `angle * π / 180`, multiplied before it is divided, with `sin` and `cos` from `libm`.
- **The reprojection.** On a rotated map, `proj`'s reprojection must hold the geotransform on the
  map's side. It must move a feature's point by the PROJ step, then by the inverse geotransform.
  For the area asked, it must sample the faked extent `0, 0, width, height` as
  `msProjectRectAsPolygon` samples it. It must move each sample by the geotransform, then by the
  PROJ step.
- **The tests that read it.** `rect.rs` and `shape.rs` must read the geotransform wherever
  MapServer's `msProjectRect`, `msProjectShapeLine`, and `msProjectGetLineCuttingCase` read
  `gt.need_geotransform`.
- **Which layers.** On a rotated map, the engine must reproject every layer where both the layer and
  the map have parameters. That includes a layer whose proj4 string does not differ from the
  map's, and a layer of `AUTO` under a map of `AUTO`. For such a layer, `proj` must run no formula
  in the PROJ step.
- **Without a projection.** The engine must not reproject a layer where it or the map has no
  parameters, as `msProjectionsDiffer` decides. The engine must draw that layer's features in the
  faked extent as the source delivers them. The engine must ask its source for the faked extent
  itself. The engine must warn `rotation-without-projection` once per such layer, naming it.
- **The frame.** After the reprojection, the engine must draw a rotated map in a `View` of extent
  `0, 0, width, height` and cellsize 1. The engine must clip, evaluate `GEOMTRANSFORM` with its
  `[map_cellsize]`, and place labels in that frame.
- **The scale.** The engine must compute the scale denominator from the grown real extent. It must
  compute the scale factor of a size in units other than pixels from `msGetGeoCellSize`'s
  cellsize.
- **No rotation.** The engine must draw a map of `ANGLE 0`, or of none, exactly as before.

**Out of scope:**

- the turn of a style's or a label's angle by the map's angle, which `pointLayerDrawShape`,
  `lineLayerDrawShape`, and `polygonLayerDrawShape` apply (the fidelity plan's ticket 14);
- the map's angle in the style IR and its format version, which ADR-0014 governs;
- `MS_NONSQUARE`, which also sets a geotransform in `msPrepareImage`;
- a scalebar, a legend, a raster layer, and the `ANGLE` parameter of a WMS request;
- G-11's reference records of ADR-0038, which hold no rotated rectangle.

## Alternatives considered

- **The rotation in the `View`.** `point_to_pixel` would turn by the angle, and the reprojection
  would stay as it is. It touches no `proj` code. Not chosen:
  - `GEOMTRANSFORM`, `[map_cellsize]`, the clip, and the label cache would work in map units, where
    MapServer works in the image's pixels;
  - the area asked would be the reprojected bounding box of the rotated view, wider than
    `msProjectRect`'s for a layer in another projection;
  - every user of the `View` in `symbolize` and `label` would learn the angle.
- **A second step in the engine, after ADR-0037's reprojection.** The engine would apply the
  inverse geotransform to the reprojected features and the geotransform before asking. Not chosen:
  `rect.rs` samples an axis-aligned rectangle inside `proj`, so the step cannot go between its
  samples and the formulas from outside. The tests that read the geotransform (*Context*, 9) would
  be repeated or dropped.
- **A layer in the map's projection run through `proj`'s formulas forward and back**, as PROJ runs
  a CRS into itself where the arguments are several. It differs from the identity by the
  formulas' round trip only. It would also send the coordinates of `renderers/symbolrot.map`
  through the harness's longitude-and-latitude stand-in for EPSG:21781, coordinates that are not
  degrees. MapServer runs no PROJ at all for those shapes (*Context*, 6).
- **A layer without a projection rotated anyway.** The engine would apply the geotransform to it,
  and ask its source for the rotated view. The output would be what a user expects. MapServer
  draws such a layer in the wrong place, and no case in scope decides it. Not chosen without the
  maintainer: G-6 holds the engine to MapServer's output. The warning departs from MapServer,
  which is silent; it tells the user why the layer lands where it does.
- **A successor of ADR-0037.** ADR-0037's decision stands unchanged. The rotation adds a MapServer
  port beside `rect.rs` and `shape.rs`, which ADR-0037 already keeps out of its scope. A new
  decision leaves ADR-0037 in force.

## Sources / Prior art

- MapServer at the pin, rel-8-6-6:
  - `src/mapobject.c`: `msMapSetRotation`, `msMapComputeGeotransformEx`,
    `msMapComputeGeotransform`, `msMapSetFakedExtent`, `msMapRestoreRealExtent`;
  - `src/mapdraw.c`: `msGetGeoCellSize`, `msPrepareImage`, `msDrawMap`, `msDrawLayer`,
    `msDrawVectorLayer`, `pointLayerDrawShape`;
  - `src/mapproject.c`: `msProjectCreateReprojector`, `createNormalizedPJ`, `msProjectPointEx`,
    `msProjectGetLineCuttingCase`, `msProjectShapeLine`, `msProjectRectAsPolygon`,
    `msProjectRect`, `msProjectionsDiffer`;
  - `src/mapfile.c`: `loadMapInternal`'s `ANGLE`; `src/mapresample.c`: `InvGeoTransform`;
    `src/mapcopy.c`: `msCopyProjectionExtended`, which copies the geotransform with `+over`;
  - `src/maputil.c`: `msAdjustExtent`; `src/mapscale.c`: `msCalculateScale`.
- The msautotest suite at the pin: `misc/rot_out.map` and its `expected/rot_out.png`,
  `renderers/symbolrot.map`, `renderers/symbolrot_negative.map`.
- Not verified: which PROJ operation PROJ 9.4.0 builds from one CRS into the same CRS for
  `createNormalizedPJ`. No PROJ is installed in the dev container. A round trip of `tmerc` stays
  far below a pixel, so it does not move the decision.

## Consequences

- Positive:
  - `misc/rot_out.map`, and the shapes of the two `symbolrot` cases, are drawn as MapServer draws
    them;
  - `symbolize` and `label` stay unchanged: a rotated map is a `View` they already draw;
  - the two `symbolrot` cases need no port of `somerc`, since no formula runs for their layers;
  - a map without `ANGLE` keeps its code path, and its cases keep their outcome.
- Negative / trade-offs:
  - on a rotated map, map units after the reprojection are the image's pixels, as in MapServer. A
    `GEOMTRANSFORM` `buffer` of 10 is 10 pixels there. A reader will take it for a defect;
  - a layer without a projection on a rotated map is drawn where MapServer draws it, which is
    wrong, with a warning;
  - a rotated map's centre sits half a pixel off an unrotated map's, as in MapServer;
  - every layer of a rotated map goes through the reprojection, at the cost of one affine step a
    point;
  - `proj` gains a third port of MapServer's code beside its port of PROJ.
- Follow-ups:
  - the glossary gains a term for the geotransform and the frame of the image, with the build;
  - `docs/semantics/` gains the map's `ANGLE`, with the build;
  - the turn of symbols and labels by the map's angle, in its own ticket.

## Enforcement

- The conformance list holds `misc/rot_out.map rot_out.png` as `geometry passes`, natively and
  through the wasm32 module (G-9).
- A unit test of `crates/proj` cites this ADR with `Verifies:`. It holds:
  - the map's and the flipped geotransform for `misc/rot_out.map`, as *Context* lists them;
  - the area asked for its faked extent, with no formula in the PROJ step;
  - the polar branches of the area asked skipped, and the dateline wrap measured without the
    geotransform, where the map holds one.
- An engine test with sources in memory cites this ADR. It holds:
  - the area asked of a source on a rotated map;
  - the pixel of a point of a layer in the map's projection, taken from the C's formulas;
  - the drawn size of a style in `SIZEUNITS meters` on a rotated map, scaled by
    `msGetGeoCellSize`'s cellsize;
  - a `GEOMTRANSFORM` `buffer` on a rotated map, its distance taken in the image's pixels;
  - `rotation-without-projection` for a layer without a projection;
  - the same image for a map of `ANGLE 0` as for a map without `ANGLE`.
- The fuzz target `fuzz/fuzz_targets/proj.rs` gains a seed with a geotransform (Q-3).
