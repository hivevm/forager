# Ticket: Place a pixmap symbol by its ANCHORPOINT

## What to build

A pixmap symbol whose `ANCHORPOINT` is not the centre is moved so that its anchor, turned with it,
lies on the point, as `msAdjustMarkerPos` (`src/maprendering.c`, 1013–1050) moves it, by the
marker's size of `msGetMarkerSize` (`src/mapsymbol.c`, 720–799), whose pixmap branch the engine
computes for the first time:

- the size is the style's `SIZE`, or the image's height without one, times the scale factor,
  truncated to an `int` (722, 750–753), then held by `MINSIZE` and `MAXSIZE` unscaled (754–755);
- at a size of 1 the marker is the image's own width and height; otherwise its height is the size
  and its width the size over the image's height times its width (772–782).

The engine's marker size of a style (today "a pixmap symbol, whose size is not computed yet") gives
that size from the decoded image's dimensions; ticket 10 uses it under a label. It lands after
`docs/plan/revision-2026-10-31b/12-symbol-resolved-once.md`, which reworks the engine's marker size
and its anchoring into one function each, so that the pixmap branch goes into those.

## Acceptance criteria

- [ ] Given a 32 × 32 pixmap with `ANCHORPOINT 0 0`, `0.2 0.7`, and `1 1` at `SIZE 10`, unturned
  and at `ANGLE 45`, when rendered, then each image is placed at the point the C's formulas give,
  stated as literals in the test, and the engine's reference image holds them.
- [ ] Given a pixmap of 40 × 20 at `SIZE 1`, at `SIZE 10.9`, and without a size at a scale factor
  of 1.5, when its marker's size is computed, then it is (40, 20), (20, 10), and (60, 30) — the
  truncation and the size-1 branch of `msGetMarkerSize` — and a test states them.
- [ ] Given `marker_anchorpoint`, when the harness renders it, then it stays `fails` and the list
  gives the reason "TrueType symbols": its classes 1 to 4 draw TrueType symbols, which Forager does
  not draw yet; the anchor point of its pixmap classes is decided by the tests above.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md) (the `int` truncation of the size);
  `docs/semantics/symbol.md` (the anchor point of a vector symbol, the golden path); the engine's
  anchoring of vector symbols and its tests; the conventions on ported behaviour and stated values.
  Seam: the engine's render tests.

## Blocked by

- docs/plan/pixmap-symbols/04-pixmap-scaled-and-rotated.md
- docs/plan/revision-2026-10-31b/12-symbol-resolved-once.md

## Mode

unattended: engine tests whose values come from the C decide each criterion; the change stays in
the engine's anchoring and marker size, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, resource path, port,
public interface, or style document format. Its branch counts against ADR-0007's cap of three open
agent branches. A wrong result shows as a stated position or size that does not hold, and as
`poly_geomtransform_anchor` failing in ticket 11; the tests citing the C prove it right. Stop: if
the C contradicts this ticket, record where here and hand off; or after three attempts at a
reference image that will not hold.
