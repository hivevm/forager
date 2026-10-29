# Ticket: Dashes, caps, joins, offsets, and outlines of lines

## What to build

The engine strokes a line style with its `PATTERN`, `LINECAP`, `LINEJOIN`, and
`LINEJOINMAXSIZE`, offsets it by `OFFSET` as MapServer's offset curve does, and draws a line's
`OUTLINEWIDTH` and `OUTLINECOLOR` under it, as MapServer's AGG renderer does at the pin.

## Acceptance criteria

- [ ] Given each of these properties, when rendered, then the image matches Forager's reference
      image, its geometry taken from MapServer's computation where one exists.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- `docs/semantics/line.md`, `docs/semantics/offset.md`, `docs/semantics/width.md`; ADR-0015.
- MapServer at the pin: `agg2RenderLine`, `applyCJC` (`src/mapagg.cpp`), `msOffsetPolyline`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when a property's meaning needs a change of the style IR, and hand off.
