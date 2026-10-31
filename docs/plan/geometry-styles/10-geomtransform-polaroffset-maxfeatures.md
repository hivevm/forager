# Ticket: GEOMTRANSFORM, POLAROFFSET, and MAXFEATURES

## What to build

The engine applies a style's `GEOMTRANSFORM` (`start`, `end`, `vertices`, `centroid`, and
the bounding box), a layer's `GEOMTRANSFORM` where it needs no geometry library, a style's
`POLAROFFSET`, and a layer's `MAXFEATURES`, as MapServer does at the pin.

## Acceptance criteria

- [ ] Given each transform, a polar offset, and a feature limit, when rendered, then the image
      matches Forager's reference, the transformed points taken from MapServer's computation.
- [ ] Given a transform that needs a geometry library, then it is a diagnostic and nothing is drawn
      in its place.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- `docs/semantics/style-symbol.md`; MapServer at the pin: `msGeomTransformShape`
  (`src/mapgeomtransform.c`), `msDrawMarkerSymbol`.

## Blocked by

- docs/plan/geometry-styles/05-vector-and-ellipse-symbols.md

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when a transform needs a change of the style IR, and hand off.

## Where it stands

The session stopped at the stop of its *Mode*. On this branch the engine draws, without a change
of the style IR, what the IR already carries:

- A style's `GEOMTRANSFORM` `start`, `end`, `vertices`, and `centroid` on a line or a polygon
  layer, with `ANGLE AUTO`, as `msDrawTransformedShape`, `calcOrientation`, `calcMidAngle`
  (`src/mapgeomtransform.c`), `msTransformShapeSimplify`, and `msGetPolygonCentroid`
  (`src/mapprimitive.cpp`) place them; the transformed styles of a cached line layer drawn with
  its first pass; `labelpnt` and `labelpoly` drawing nothing on a shape; a point layer drawing
  every style at its points whatever its transform.
- A style's `POLAROFFSET`, bound or written, and its `OFFSET`, moving a symbol at a point as
  `msAdjustMarkerPos` (`src/maprendering.c`) moves it.

What is left needs the IR's format to change (ADR-0020), which the maintainer decides (ADR-0007):

- **`bbox`.** Carried by `docs/plan/pixmap-symbols/10-geomtransform-bbox.md`, beside the pixmap
  symbols `renderers/poly_geomtransform_anchor.map` also needs; no longer this ticket's.
- **A transform that needs a geometry library** (`buffer`, `simplify`, `simplifypt`,
  `centerline`, …). The second criterion asks for a diagnostic and nothing drawn; the IR has no
  way to say "this style draws nothing", and the frontend can only drop the transform, so the
  style draws the feature's own geometry. Dropping the style from the class would change the
  count of styles, which decides whether a line layer caches its features, and the place of each
  pass; a variant of `GeomTransform` for an expression the engine does not draw (with the
  expression's text, for the diagnostic) would serve it.
- **A layer's `GEOMTRANSFORM`.** `Layer` has no field for it. MapServer reads only an expression
  there (`msGeomTransformShape`, RFC 89). Of those in the suite, `simplify` and `centerline` need
  GEOS; `generalize`, `smoothsia`, `densify`, `outer`, and `inner` do not. The one in-scope case,
  `geomtransform-vertex-layer.png`, uses `simplify`.
- **`MAXFEATURES`** of a layer, and the metadata `maxfeaturestodraw` and the output format's
  `MAXFEATURESTODRAW` that `misc/maxfeatures.map` also exercises: `Layer` has no field for them.
  Its inline `FEATURE` points are drawn since ADR-0033.

`point-polaroffset.png` passes since inline features are drawn; `geomtransform-vertex.png` still
needs the transform expressions of a style and TrueType symbols, as its reason in `cases.txt`
says.
