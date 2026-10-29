# Ticket: A label sits and looks where MapServer puts it

## What to build

A label honours `POSITION` (the nine positions, and `AUTO` for a point), `OFFSET` in pixels,
`ANGLE`, `ALIGN` and `WRAP` for several lines, `SIZE` in points scaled as MapServer scales it, and
`OUTLINECOLOR` with `OUTLINEWIDTH`, each as MapServer computes it at the pin, bound or literal.

## Acceptance criteria

- [ ] Given a label at each of the nine positions, when rendered, then its box stands where
      MapServer's computation at the pin puts it, to the pixel, in a test of the placement alone.
- [ ] Given an `OFFSET`, an `ANGLE`, and a wrapped text with each `ALIGN`, then the same holds.
- [ ] Given an outline, then the text is drawn with it, and the image matches Forager's reference.

## Read first

- The spec beside this ticket; MapServer at the pin, where a label's point and box are computed.
- The engine's render tests and the tests of ticket 04.

## Blocked by

- docs/plan/labels/04-one-label-at-a-point.md
- docs/plan/revision-2026-10-24/26-label-bindings-as-mapserver-binds-them.md

## Mode

unattended: tests of the placement and the engine decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of ADR-0007. Stop when a
property's meaning in MapServer needs a change of the style IR, and hand off.
