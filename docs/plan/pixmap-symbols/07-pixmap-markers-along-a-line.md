# Ticket: Place pixmap symbols along a line at its GAP

## What to build

A line style whose symbol is a pixmap and whose `GAP` is not 0 places the image along the line, as
`msDrawLineSymbol` (`src/maprendering.c`, 600–731) hands it to `msImagePolylineMarkers` (388–598):
turned with each segment for a negative `GAP`, unturned for a positive one (679–686), the gap
scaled as the size is (`computeSymbolStyle`, 66). `msImagePolylineMarkers` takes the marker's
width and height as the image's, times the scale, at least 1 (400–401); anchors each marker by
them (447–464); skips one whose point lies outside the image grown by that width and height
(467–474); places one marker halfway along a line on which none fell, without `INITIALGAP`, where
the line is longer than the marker is wide (524–595); and draws the image as at a point
(478–480, 559–561). The engine's placement of vector symbols along a line already follows this
function; it takes the pixmap's size and draws the image of tickets 02 and 03. The markers count
towards the layer's marker budget (Q-9), as today.

## Acceptance criteria

- [ ] Given a line with a 32 × 32 pixmap at `GAP 50`, at `GAP -50` with `SIZE 16`, and on a line
  too short for a gap, when rendered, then the markers lie where the C's formulas place them,
  stated as literals in the test, and the engine's reference images hold them.
- [ ] Given a pixmap marker with `ANCHORPOINT 0 0` along a line, when rendered, then each is
  anchored by the scaled image's width and height as lines 447–464 anchor it.
- [ ] Given `line_marker_pixmap`, when the harness renders it, then it passes and the list says so.

## Read first

- `docs/semantics/line.md` (`GAP`, `INITIALGAP`, the marker budget); the engine's symbols along a
  line and its tests, the golden path; the conventions on ported behaviour and on what a render
  holds per feature. Seams: the engine's render tests, the harness.

## Blocked by

- docs/plan/pixmap-symbols/03-pixmap-scaled-and-rotated.md

## Mode

unattended: engine tests whose values come from the C, and the harness, decide each criterion;
the change stays in the engine's markers along a line, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: it keeps the marker budget as it is and
sets no limit, path, port, interface, or format. Its branch counts against ADR-0007's cap of three
open agent branches. A wrong result shows as a stated position that does not hold or a case that
does not pass; the tests citing the C prove it right. Stop: if the engine's placement along a line
departs from `msImagePolylineMarkers` in a way this ticket would have to change for vector symbols
too, record it here and hand off; or after three attempts at the case.
