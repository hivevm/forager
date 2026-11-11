# Ticket: MAXFEATURES, a style transform drawn as nothing, and a layer's GEOMTRANSFORM

## What to build

A style's `GEOMTRANSFORM` `start`, `end`, `vertices`, and `centroid` and its `POLAROFFSET` landed
in #115 (b7243d4, `crates/symbolize/src/geom_transform.rs`), and `bbox` with the ticket
`geomtransform-bbox/01`; this ticket is narrowed to what is left,
each part of which needs the style IR's format to change (ADR-0014):

- **A layer's `MAXFEATURES`,** and the metadata `maxfeaturestodraw` and the output format's
  `MAXFEATURESTODRAW` that `misc/maxfeatures.map` also exercises: `Layer`
  (`crates/style-ir/src/model.rs`) has no field for them, and the frontend lists `MAXFEATURES`
  among the keywords it does not carry over (`crates/mapfile/src/syntax.rs`). The map's inline
  `FEATURE` points are drawn since ADR-0018.
- **A style transform that needs a geometry library** (`buffer`, `simplify`, `simplifypt`,
  `centerline`, …) draws nothing, with a diagnostic. Today `geom_transform`
  (`crates/mapfile/src/translate/style.rs`) warns and drops the transform, so the style draws the
  feature's own geometry. Dropping the style from the class would change the count of styles,
  which decides whether a line layer caches its features, and the place of each pass; a variant
  of `GeomTransform` for an expression the engine does not draw, with the expression's text for
  the diagnostic, would serve it.
- **A layer's `GEOMTRANSFORM`.** `Layer` has no field for it. MapServer reads only an expression
  there (`msGeomTransformShape`, RFC 89). Of those in the suite, `simplify` and `centerline` need
  GEOS; `generalize`, `smoothsia`, `densify`, `outer`, and `inner` do not. The one in-scope case,
  `geomtransform-vertex-layer.png`, uses `simplify`, so it would draw nothing under the variant
  above.

## Acceptance criteria

- [ ] Given `misc/maxfeatures.map`, when rendered, then the image matches Forager's reference,
      the features drawn per layer being those MapServer's limit leaves.
- [ ] Given a style whose `GEOMTRANSFORM` needs a geometry library, when rendered, then it is a
      diagnostic naming the expression, nothing is drawn in its place, and the other styles of
      its class draw in the same passes as before.
- [ ] Given a layer `GEOMTRANSFORM` that needs no geometry library, when rendered, then the
      transformed shapes match MapServer's computation; one that needs it draws nothing, with a
      diagnostic.
- [ ] Given the style document of each new field, when written and read back, then it round
      trips, and the JSON Schema under `schema/` carries it with the version bump ADR-0014 asks.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in
      `crates/cli/tests/conformance/cases.txt`.

## Read first

- [ADR-0014](../../adr/0014-style-document-versions.md),
  [ADR-0018](../../adr/0018-inline-features.md); `docs/semantics/style-symbol.md`.
- `crates/symbolize/src/geom_transform.rs`, `crates/mapfile/src/translate/style.rs`
  (`geom_transform`), `Layer` and `GeomTransform` in `crates/style-ir/src/model.rs`.
- MapServer at the pin: `msGeomTransformShape` (`src/mapgeomtransform.c`), `msDrawMarkerSymbol`,
  and its reading of `MAXFEATURES` and `MAXFEATURESTODRAW`.

## Blocked by

- None (can start immediately)

## Mode

with the human: every part changes the style document format, which
[ADR-0006](../../adr/0006-harness.md) never leaves unattended. The maintainer decides the shape of
the new fields and the variant, and the version bump they take.

## Maintainer's decision (2026-11-10)

As described: a layer field for `MAXFEATURES` (with `maxfeaturestodraw` and the output format's
`MAXFEATURESTODRAW`); a variant of `GeomTransform` for a style transform the engine does not draw,
which draws nothing with a diagnostic carrying the expression's text; a layer `GEOMTRANSFORM` field,
whose transforms without GEOS (`generalize`, `smoothsia`, `densify`, `outer`, `inner`) are ported
and whose others draw nothing with that diagnostic. Every addition or narrowing of the style
document keeps the format at 1.0 until its first release, the schema regenerated (ADR-0014's freeze
while unreleased).
