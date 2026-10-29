# Ticket: Hatch symbols fill polygons

## What to build

The engine fills a polygon with a hatch symbol at its `SIZE`, `WIDTH`, and `ANGLE`, and with a
`PATTERN` on the hatch lines, as MapServer's `msHatchPolygon` and its AGG renderer do at the pin.

## Acceptance criteria

- [ ] Given a hatch at two angles and with a pattern, when rendered, then the lines stand where
      `msHatchPolygon` puts them, in a test of their own, and the image matches Forager's
      reference.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- `docs/semantics/symbol.md`; MapServer at the pin: `msHatchPolygon` (`src/maprendering.c`).

## Blocked by

- docs/plan/geometry-styles/07-line-styles.md

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when the hatch needs a change of the style IR, and hand off.
