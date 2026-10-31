# Ticket: Scale and turn a pixmap symbol at a point

## What to build

A pixmap symbol at a point drawn at the style's `SIZE` and `ANGLE`, as MapServer draws it:

- `computeSymbolStyle` (`src/maprendering.c`, 37–79): the scale is the style's size, or the
  image's height without one (`msSymbolGetDefaultSize`, `src/mapsymbol.c`, 74–79), times the scale
  factor, bounded by `MINSIZE` and `MAXSIZE` times the resolution factor, over the image's height
  (62–65); the rotation is `ANGLE` in radians. A scale of 0 draws nothing
  (`msDrawMarkerSymbolInternal`, 935–937).
- `agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 646–702): where the rotation is neither 0 nor 2π or
  the scale is not 1, the square of half-side `MS_NINT(max(width, height) * scale * 1.415) / 2 + 1`
  (integer division) about the point is filled non-zero, without gamma, by AGG's
  `span_image_filter_rgba_bilinear_clip` with a transparent background, through
  `span_interpolator_linear` over the inverse of: translate by minus half the image, rotate by
  `-rotation`, scale, translate to the point (656–690). The reference renderer draws it with
  agg-rust's ports of those span generators; the square is clipped as ADR-0039 clips a path.

## Acceptance criteria

- [ ] Given a pixmap at `SIZE 55 ANGLE 45` and one at `SIZE 16` unturned, when rendered, then the
  engine's reference images hold them, and a test states the square's half-side and the inverse
  transform's six coefficients as literals computed from the C's formulas.
- [ ] Given a pixmap at `ANGLE 360`, when rendered, then it is drawn by the blend of ticket 02 only
  where the rotation equals 2π exactly in radians, as the C compares it, and resampled otherwise;
  the test states which.
- [ ] Given `MINSIZE` and `MAXSIZE` and a `RESOLUTION` over `DEFRESOLUTION` of 2, when rendered,
  then the scale is bounded as `computeSymbolStyle` bounds it, the value stated in the test.
- [ ] Given a scale that places the square far outside the image, when rendered, then nothing is
  drawn and nothing panics.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md); ADR-0028 is not
  touched.
- `docs/semantics/symbol.md`, `docs/semantics/scale.md`; the conventions on ported behaviour and
  on tests that hold stated values. Golden path: the vector symbol at a point in the engine and its
  reference images. Seam: the engine's render tests.

## Blocked by

- docs/plan/pixmap-symbols/02-png-point-marker.md

## Mode

unattended: engine tests whose values come from the C decide each criterion; the change stays in
the engine's point marks and the reference renderer's image drawing, undone by one revert. It
touches nothing [ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit,
resource path, port, public interface, or style document format. Its branch counts against
ADR-0007's cap of three open agent branches. A wrong result shows as a reference image or a stated
coefficient that does not hold, and as the cases of tickets 05, 08, and 09 failing; the tests citing
the C prove it right. Stop: if agg-rust's bilinear clip filter or its interpolator differs from
AGG 2.4's in what it computes, record where here and hand off, or after three attempts at a
reference image that will not hold.
