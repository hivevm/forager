# Ticket: Symbols are placed along lines

## What to build

The engine places a line style's symbol along each line by `GAP` and `INITIALGAP`, rotated with
the line where the gap is negative or as MapServer does, as `msDrawLineSymbol` and its marker
placement do at the pin.

## Acceptance criteria

- [ ] Given a line with a symbol and a positive and a negative `GAP`, and an `INITIALGAP`, when
      rendered, then the symbols stand where MapServer's placement puts them, in a test of the
      placement alone, and the image matches Forager's reference.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- The tests of ticket 05; MapServer at the pin: `msDrawLineSymbol`, `msImagePolylineMarkers`
  (`src/maprendering.c`).

## Blocked by

- docs/plan/geometry-styles/05-vector-and-ellipse-symbols.md

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when MapServer's placement needs a change of the style IR, and hand off.
