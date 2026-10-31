# Ticket: Place a label beside a pixmap marker by the image's size

## What to build

A point label whose class's first style draws a pixmap is placed and tested against the marker
the label cache holds for it, sized as MapServer sizes it, instead of being left out with
`label-not-drawn` ("a pixmap symbol, whose size is not computed yet"). `msAddLabelGroup` and
`msAddLabel` (`src/maplabel.c`, 343–376 and 590–626) hold the marker of a point layer's label as
the box of `msGetMarkerSize` (`src/mapsymbol.c`, 720–799) of the class's first style about the
point, which loads the image where it has not been (736–738); the engine holds and uses that box
for a vector symbol already, and takes the pixmap's size of ticket 07 for it. The image is loaded
before the label needs its size.

A label's own `labelpnt` style whose symbol is a pixmap (`computeMarkerBounds`, `src/mapdraw.c`,
3284 onwards) stays out of scope: `marker_pixmap`'s one label style is a `labelpoly`.

**Decided by the maintainer: the label of a pixmap that does not load is left out.** MapServer's
failing `msGetMarkerSize` fails `msAddLabelGroup` (350–352) and `msAddLabel` (606–608), and so the
layer and the map. Forager leaves the label out with the warning of the pixmap that does not load,
as it leaves the style undrawn, and draws the rest of the map; the label cache never places a label
MapServer would not have placed.

## Acceptance criteria

- [ ] Given a point with a 32 × 32 pixmap marker and a label at `POSITION lc`, unscaled and at
  `SIZE 55 ANGLE 45`, when rendered, then the marker the cache holds is the box the C's formulas
  give, stated as literals in the test, and the label is placed or discarded against it as the
  engine's label tests state.
- [ ] Given a label whose class's first style is a pixmap that does not load, when rendered, then
  the label is left out, absent from the label cache's labels, the warning names the symbol and its
  path, and the rest of the map is drawn.
- [ ] Given `marker_pixmap`, when the harness renders it, then it passes and takes its
  `labels-hold` mark, both of its labels discarded as MapServer's record says.
- [ ] Given the label memory test, when it runs with a pixmap marker under each label, then each
  label still keeps to Q-9's per-label bound, the image held once per render and not per label.

## Read first

- [ADR-0028](../../adr/0028-text-stack.md),
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md); `docs/semantics/label.md` (the marker
  under a label); the convention on what a render holds per label; the engine's label tests and
  `crates/engine/tests/label_memory.rs`, the seams; the conformance record of `marker_pixmap`'s
  labels.

## Blocked by

- docs/plan/pixmap-symbols/07-pixmap-anchor-point.md

## Mode

unattended: engine tests whose values come from the C, the label memory test, and the harness
decide each criterion; the change stays in the engine's labels, undone by one revert. It touches
nothing [ADR-0007](../../adr/0007-harness.md) keeps with the human: it sets no limit and keeps the
per-label bound of Q-9 as it is, and touches no path, port, interface, or format; the one behaviour
that was open, the label of a pixmap that does not load, the maintainer decided. Its branch counts
against ADR-0007's cap of three open agent branches. A wrong result shows as a stated box that does
not hold, a `labels-hold` mark that moves, a label present that should be left out, or the memory
test failing, and is undone by the revert; the tests citing the C prove it right. Stop: if holding
the image's size per label would grow what a cached label keeps past Q-9's bound, or the label
stage needs the image loaded where the engine cannot await it, record why here and hand off.
