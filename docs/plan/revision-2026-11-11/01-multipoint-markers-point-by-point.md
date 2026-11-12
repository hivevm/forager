# Ticket: Draw a point layer's multipoint marker by marker per point under LABELCACHE ON too

## What to build

A point layer draws a multipoint's markers in two orders today. Under `LABELCACHE OFF`, the engine
draws every style's marker at one point before the next point (`draw_points` in
`crates/engine/src/lib.rs`). Under `LABELCACHE ON`, `Drawing::draw` hands the feature to
`symbolize::draw_ops`, which draws one style at every point before the next style.
`pointLayerDrawShape` (`src/mapdraw.c:2062-2105`) draws point by point in both modes, so the
overlapping markers of a multipoint with two or more styles stack differently from MapServer's.
Draw a point layer's features point by point whatever its `LABELCACHE`, through one path: the
`LABELCACHE OFF` path without its label step. The design revision of 2026-11-11 found the order
written twice; PR #176 named the divergence among its lesser findings.

## Acceptance criteria

- [ ] Given a point layer of `LABELCACHE ON` with a multipoint of two points 5 px apart and two
  styles whose markers overlap, when it is rendered, then each point's markers are drawn before
  the next point's, as the order of the drawing operations shows, each marker's x stated.
- [ ] Given the engine, when a point layer's features are drawn, then one function decides the
  order of their markers for both values of `LABELCACHE`.
- [ ] Given the conformance suite, when it runs, then no case in scope passes less than before.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/plan.rs`, `crates/engine/tests/`
- `docs/semantics/label.md` (the paragraph on `LABELCACHE OFF`), `docs/ARCHITECTURE.md`
  (`engine`, and *How it runs*), [ADR-0008](../../adr/0008-ports-and-adapters-crates.md).
- Overlap: `docs/plan/postlabelcache/01-postlabelcache.md` draws labels at once too; whichever
  lands second takes the other's path.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests above decide the criteria; the change stays in `crates/engine` and costs
one revert; it touches nothing ADR-0006 keeps with the human. A wrong result shows as a changed
conformance case or a failing test of the engine. Stop after two attempts at a case of the
conformance suite that turns red, and hand off with the case named.
