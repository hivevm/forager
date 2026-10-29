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
