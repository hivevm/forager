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

**Question for the maintainer: the label of a pixmap that does not load.** MapServer's failing
`msGetMarkerSize` fails `msAddLabelGroup` (350–352) and `msAddLabel` (606–608), and so the layer
and the map. The maintainer's decision for a pixmap that does not load leaves its style undrawn
and the rest of the map drawn; it does not say what becomes of the label beside it. Three ways:
leave the label out with a diagnostic, as the engine leaves out a label whose marker it cannot
size today; place it as though its class had no marker; or place it against a marker of size 1, as
`msGetMarkerSize` gives for symbol 0. The proposal is the first, which keeps the label cache from
placing a label MapServer never would. The ticket's second criterion takes the answer before it
starts.

## Acceptance criteria

- [ ] Given a point with a 32 × 32 pixmap marker and a label at `POSITION lc`, unscaled and at
  `SIZE 55 ANGLE 45`, when rendered, then the marker the cache holds is the box the C's formulas
  give, stated as literals in the test, and the label is placed or discarded against it as the
  engine's label tests state.
- [ ] Given a label whose class's first style is a pixmap that does not load, when rendered, then
  the label is treated as the maintainer's answer to the question above says, the diagnostic
  naming the symbol and its path, and the rest of the map is drawn.
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

with the human: what becomes of a label beside a pixmap that does not load is a behaviour the
maintainer's decision on failed loads does not cover, and its question above is his to answer
before the ticket starts. Once answered, the rest would meet the unattended conditions: engine
tests whose values come from the C, the label memory test, and the harness decide each criterion,
the change stays in the engine's labels and is undone by one revert, and it sets no limit, path,
port, interface, or format. It counts against [ADR-0007](../../adr/0007-harness.md)'s cap of three
open agent branches. A wrong result shows as a stated box that does not hold, a `labels-hold` mark
that moves, or the memory test failing. Stop: if holding the image's size per label would grow
what a cached label keeps past Q-9's bound, or the label stage needs the image loaded where the
engine cannot await it, record why here and hand off.
