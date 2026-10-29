# Ticket: Features are drawn with the class their expression selects, past the layer's filter

## What to build

The `expr` crate evaluates the IR's tree as the ADR of ticket 01 decides, and the engine draws
each feature with the first class whose `EXPRESSION` matches it, after the layer's `FILTER`, as
MapServer's `msShapeGetClass` and `msLayerWhichShapes` select. Bound and computed style
properties are evaluated per feature.

## Acceptance criteria

- [ ] Given classes with expressions of each kind, when rendered, then each feature is drawn with
      the first class that matches, and a feature no class matches is not drawn.
- [ ] Given a layer `FILTER`, then features that fail it are not drawn.
- [ ] Given a style `SIZE`, `WIDTH`, or `COLOR` bound to an attribute or computed by an
      expression, then each feature is drawn with its own value.
- [ ] Given the harness, then every case this makes pass is flipped to `passes` in the list.

## Read first

- The ADR of ticket 01; ADR-0009, ADR-0019, ADR-0026; the engine's render tests.
- MapServer at the pin: `msShapeGetClass` (`src/maputil.c`).

## Blocked by

- docs/plan/geometry-styles/01-evaluator-adr.md

## Mode

unattended: the engine's tests and the harness decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open agent branches
of ADR-0007. Stop when an operator's meaning in MapServer needs a change of the
style IR, and hand off.
