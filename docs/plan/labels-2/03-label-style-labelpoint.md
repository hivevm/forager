# Ticket: A label's labelpoint and labelcenter styles, and the box the report gives

## What to build

A label's styles with `GEOMTRANSFORM labelpoint` (a marker at the label's point) and `labelcenter`
(at its box's centre) are drawn as `msDrawLabelCache` draws them and their bounds count in the
label's collisions; the box the engine reports in `Rendered.labels` for a drawn label is MapServer's
member box, the union of the text's box and the `labelpoint` styles' bounds, as the maintainer
chose, so that a case with such styles can hold G-10. `labelpnt` and `marker_pixmap`'s vector part
come closer; the cases that only lacked these hold their new outcome.

## Acceptance criteria

- [ ] Given a label with a `labelpoint` style, when the map renders, then the marker is drawn at the
  label's point under the text, matching a reference worked out from the C.
- [ ] Given that label drawn, then its reported box is the union of the text's box and the marker's
  bounds, as `msDrawLabelCache` forms the member's `bbox` (a test whose expected box is computed
  from the C's formula by hand).
- [ ] Given the harness, then each case this changes holds its new listed outcome and records
  comparison; the 5 marked cases stay marked.

## Read first

- `src/mapdraw.c` at the pin (`msDrawLabelCache`, the global label bbox and the LABELPOINT styles);
  ticket 02 of this spec; ADR-0029.

## Blocked by

- docs/plan/labels-2/02-label-style-labelpoly.md

## Mode

unattended: the reference images, the label report's test and the harness decide its criteria; its
scope is the engine's labels; a wrong result costs one revert; it touches none of the parts
ADR-0007 keeps with the maintainer and counts against its cap of three; stop: after two attempts
that leave `scripts/check-all.sh` red, or when a case that passed before falls, the session leaves
its state in this file and hands off.
