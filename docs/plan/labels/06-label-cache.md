# Ticket: Labels that collide are left out as MapServer leaves them out

## What to build

The engine collects the labels of all layers and places them after the layers are drawn, as
MapServer's label cache does at the pin: by `PRIORITY`, then in MapServer's order; a label whose
box, grown by `BUFFER`, meets one already placed, or leaves the image, is discarded unless `FORCE`
or `PARTIALS` says otherwise. A label's `EXPRESSION` decides whether it is drawn at all.

## Acceptance criteria

- [ ] Given two labels whose boxes overlap, when rendered, then the one MapServer places first stays
      and the other is discarded, in a test of the cache alone.
- [ ] Given priorities, a buffer, `FORCE`, and `PARTIALS` at the image edge, then each decides as
      MapServer's label cache does at the pin.
- [ ] Given a label whose `EXPRESSION` does not match the feature, then it is not drawn.

## Read first

- The glossary's *Label cache*; MapServer at the pin, its label cache and the order it places in.
- The tests of tickets 04 and 05.

## Blocked by

- docs/plan/labels/05-position-offset-outline.md
- docs/plan/revision-2026-10-24/27-label-expression-fails-closed.md

## Mode

unattended: tests of the cache and the engine decide every criterion; undone by one revert; nothing
ADR-0007 keeps with the maintainer. It counts against the cap of ADR-0007. Stop when MapServer's
order cannot be followed from its source without a change of the style IR, and hand off.
