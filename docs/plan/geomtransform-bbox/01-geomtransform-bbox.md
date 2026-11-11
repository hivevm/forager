# Ticket: Draw a style on its feature's bounding box (GEOMTRANSFORM bbox)

## What to build

A style's `GEOMTRANSFORM bbox` draws the style on the bounding box of the feature's shape in
pixels, as MapServer draws it. `msStyleSetGeomTransform` (`src/mapgeomtransform.c`, 48–49) reads a
value beginning with `bbox`; `msDrawTransformedShape` (161–188) computes the shape's bounds, holds
each side within the image grown by the padding `MS_MAX(style->width, style->size) + 3`, converted
to an `int` (165–166), as a closed ring of five points (170–187), and hands it to
`msDrawShadeSymbol` (188), which strokes it as a line where the style has an outline colour and no
colour, and fills it otherwise (`src/maprendering.c`, 753–763 and onwards).

The style IR has no value for it: the frontend warns and the style draws the feature's own
geometry. The transform gains a value `bbox` in the style document, which is a change of its
format: under [ADR-0014](../../adr/0014-style-document-versions.md) the
maintainer decides whether a reader of an older minor, which drops the unknown value and draws the
feature's own geometry, fails closed enough for a minor version, or whether the change is major.
The schema under `schema/` moves with it. This ticket alone carries `bbox`;
`docs/plan/maxfeatures-and-layer-geomtransform/01-maxfeatures-and-layer-geomtransform.md` points
here.

## Acceptance criteria

- [ ] Given a style with `GEOMTRANSFORM bbox`, `'BBOX'`, and `'bboxes'`, when loaded, then each is
  the transform `bbox` in the style IR, with no warning, and the document round-trips (G-3).
- [ ] Given a document of the format version this change makes, read by the reader of the version
  before it, when it meets `bbox`, then it behaves as ADR-0014 and the maintainer's decision
  require, a test stating which.
- [ ] Given a line feature whose bounds reach past the image, with `WIDTH 5` and an outline colour,
  when rendered, then the stroked ring's corners are the bounds held by the padding, stated as
  literals from the C's formula in the test, and the engine's reference image holds it.
- [ ] Given `poly_geomtransform_anchor`, when the harness renders it, then it passes and the list
  says so.

## Read first

- [ADR-0006](../../adr/0006-harness.md), [ADR-0013](../../adr/0013-style-document-serde.md),
  [ADR-0014](../../adr/0014-style-document-versions.md),
  [ADR-0027](../../adr/0027-clip-to-the-image.md).
- `docs/semantics/style-symbol.md` (`GEOMTRANSFORM`); the engine's transformed styles and their
  reference images, the golden path; the style IR's schema test. Seams: the frontend's and the
  style IR's tests, the engine's render tests, the harness.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document format and its version, which
[ADR-0006](../../adr/0006-harness.md) never lets run unattended.

## Maintainer's decision (2026-11-10)

As described. Every addition or narrowing of the style document keeps the format at 1.0 until its
first release, the schema regenerated (ADR-0014's freeze while unreleased).
