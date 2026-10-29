# Ticket: A layer's inline features are drawn

## What to build

The frontend reads a layer's `FEATURE` blocks into the style IR as the ADR of ticket 03 decides,
and the engine draws them as features of the layer's source, with their `ITEMS` as properties.

## Acceptance criteria

- [ ] Given `FEATURE POINTS … END` for a point, a line, and a polygon, and a `WKT`, when
      rendered, then each is drawn as MapServer draws it.
- [ ] Given `ITEMS`, then a class expression reads them as properties.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- The ADR of ticket 03; the frontend's translation and the engine's render tests.

## Blocked by

- docs/plan/geometry-styles/03-inline-features-adr.md

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when MapServer's reading of a `FEATURE` needs a change the ADR did
not decide, and hand off.
