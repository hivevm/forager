# Ticket: Vector and ellipse symbols are drawn at points

## What to build

The engine draws a style's vector or ellipse symbol at each point, filled or stroked, sized by
`SIZE`, rotated by `ANGLE`, and anchored by its `ANCHORPOINT`, as MapServer's AGG renderer
draws them at the pin.

## Acceptance criteria

- [ ] Given a vector symbol, filled and not, and an ellipse, at a size and an angle, when
      rendered, then each matches Forager's reference image, its geometry taken from MapServer's
      computation in a test of its own.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- `docs/semantics/symbol.md`, `docs/semantics/size.md`, `docs/semantics/angle.md`; ADR-0015.
- MapServer at the pin: `msDrawMarkerSymbol` (`src/maprendering.c`), `agg2RenderVectorSymbol`,
  `agg2RenderEllipseSymbol` (`src/mapagg.cpp`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when a symbol's meaning needs a change of the style IR, and hand off.
