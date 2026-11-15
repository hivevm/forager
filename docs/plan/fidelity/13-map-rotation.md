# Ticket: The map's `ANGLE` rotates the map

## What to build

A map with an `ANGLE` is drawn rotated by that many degrees, as MapServer draws it, natively and on
wasm32. This ticket carries the rotation through the extent, the area asked of a source, and the
shapes. Ticket 14 turns symbols and labels with it.

MapServer does the rotation in four steps:

1. `loadMap` reads `ANGLE` and calls `msMapSetRotation` (`src/mapobject.c:350`), which sets
   `gt.need_geotransform` and computes the geotransform (`msMapComputeGeotransform`, `:381`/`:421`).
2. `msPrepareImage` fakes the extent for the rotated view (`msMapSetFakedExtent`,
   `src/mapobject.c:465`, called at `src/mapdraw.c:240`) and restores it afterwards
   (`msMapRestoreRealExtent`, `:513`).
3. With a geotransform, `msProjectionsDiffer` (`src/mapproject.c`, its `gt` test) finds every layer
   different from the map, so every layer reprojects. The geotransform is applied inside the
   reprojection (`src/mapproject.c:880-1113`, the `gt` branches).
4. The cellsize that scales the styles is taken from the saved extent (`msGetGeoCellSize`,
   `src/mapdraw.c:43`).

So the rotation sits inside the seam ADR-0037 decides: the reprojection of a layer's features and of
the area asked of its source in `crates/engine`. **The session first decides with the maintainer
whether this fits ADR-0037 or changes its seam.** If the rotation becomes a second step after the
reprojection, or a path for layers in the map's projection that ADR-0037 does not name,
`propose-adr` runs before anything is built.

The map's angle enters the style IR as an optional field of the map, an addition the version rule of
ADR-0014 allows as a minor change. No release tag holds a schema yet, so the change regenerates the
current version's file, unless the maintainer moves the version.

Case: `misc/rot_out.map rot_out.png`: a polygon shapefile in UTM zone 11, map and layer in the same
projection, `ANGLE 30`. `proj` implements `tmerc` for UTM.

## Acceptance criteria

- [ ] Given `misc/rot_out.map`, when the harness renders `rot_out.png`, then it passes by the metric
  of G-6, natively and bit-identically on wasm32 (G-9), and the list says `geometry passes`.
- [ ] Given a map of `ANGLE a`, when the frontend loads it, then the style IR holds the angle, and
  the style document round-trips (G-3). A map of `ANGLE 0` or none holds no field.
- [ ] Given a rotated map, when the engine asks a layer's source for its features, then the area
  asked is the one `msMapSetFakedExtent` and `msProjectRect` give for the rotated view. A test of
  the engine's render with sources in memory decides it.
- [ ] Given a rotated map and a layer in the map's projection, when it is drawn, then each point
  lands where MapServer's geotransform puts it, the expected pixels taken from the C at the pin.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome. A map without
  `ANGLE` draws exactly as before.

## Read first

- Touches: `crates/style-ir/src/model.rs` (`Map`), `schema/style-document-1.1.json`,
  `crates/mapfile/src/translate/mod.rs` (the map's keywords), `crates/engine/src/lib.rs` (`view`,
  `in_view`, `asked`), `crates/engine/src/reproject.rs`, `crates/symbolize/src/extent.rs` (`View`),
  `crates/proj/src/rect.rs`, `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapobject.c` (`msMapSetRotation`, `msMapComputeGeotransformEx`,
  `msMapSetFakedExtent`, `msMapRestoreRealExtent`), `src/mapdraw.c` (`msPrepareImage`,
  `msGetGeoCellSize`), `src/mapproject.c` (`msProjectionsDiffer`, the `gt` branches of point and
  rect projection), `src/mapfile.c` (`loadMap`'s `ANGLE`)
- `docs/semantics/projection.md`, `docs/semantics/scale.md`, `docs/semantics/angle.md`
- `docs/adr/0037-reprojection-ported-in-proj.md` (*Decision*, *Applies to*),
  `docs/adr/0039-declared-projection.md`, `docs/adr/0014-style-document-versions.md`
- Skill: `propose-adr`, if the seam changes
- Golden path: the reprojection of a layer in `crates/engine/src/reproject.rs` and
  `crates/engine/tests/reproject.rs`
- Seam: the msautotest harness; the engine's render with sources in memory; the frontend's `load`
  with files in memory; `crates/style-ir/tests/schema.rs`

## Blocked by

- docs/plan/fidelity/01-g7-leaves-the-specification.md
- docs/plan/fidelity/02-g14-browser-test.md
- docs/plan/fidelity/03-diagnostics-named-by-tests.md
- docs/plan/fidelity/04-non-goal-cases-out-of-scope.md
- docs/plan/fidelity/05-truetype-symbol-at-a-point.md
- docs/plan/fidelity/06-truetype-symbols-anchored-along-lines-and-as-fills.md
- docs/plan/fidelity/07-vector-symbol-as-polygon-fill.md
- docs/plan/fidelity/08-circle-layers.md
- docs/plan/fidelity/09-style-antialias-false.md
- docs/plan/fidelity/10-style-geomtransform-expressions.md
- docs/plan/fidelity/11-simplify-through-a-geometry-library.md
- docs/plan/fidelity/12-geomtransform-buffer.md

## Mode

The maintainer asked for the map's `ANGLE` as the last work of this plan, so this ticket is blocked
by every other ticket.

with the human: it changes the style document, and it may change the seam of ADR-0037's
reprojection. ADR-0006 keeps both with the maintainer. The session stops after it has read the C and
written down where the rotation sits. It hands that to the maintainer, with an ADR if the seam
changes, before it builds. It also stops when the case still fails after two attempts at its
remaining difference.
