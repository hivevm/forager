# Ticket: Pin the values the resolution, the transformed marker, and the label report tests only relate

## What to build

Three tests of today's work hold a value the code under test computes, or only a relation between
two of its outputs, which a wrong implementation that both share passes (the convention on tests
that hold a value they state, in `docs/CONVENTIONS.md`):

- `at_a_factor_a_point_draws_its_symbol_scaled_and_its_stand_in_square_as_written`
  (`crates/engine/src/draw.rs`) takes its expected display items for a `GEOMTRANSFORM start` from
  `symbol::symbol_ops` itself, so it tests the wiring and not the placement.
- `at_resolution_144_a_marker_is_drawn_twice_its_size_at_72` (`crates/engine/tests/render.rs`) and
  `at_resolution_144_a_label_moves_away_from_the_marker_as_it_is_drawn`
  (`crates/engine/tests/labels.rs`) compare the render at 144 with one doubled at 72; a scaling
  wrong on both sides passes, and `renderers/resolution.map`, which would judge it, is outside the
  harness. #136's review checked its point layer pixel-exact by hand at 72, 144, and 288.
- `the_render_reports_its_labels_drawn_and_discarded_in_the_caches_order`
  (`crates/engine/tests/labels.rs`) asserts only that a drawn label has a box and that its ink lies
  inside it, with one priority and no MultiPoint, though its comment says it holds the ink to the
  pixel and the order of the cache's walk.

Give each a value it states, taken from MapServer's arithmetic read at the pin, or from a render of
`map2img` the pull request names, beside the relation where the relation says something more.

## Acceptance criteria

- [ ] Given the transformed marker test, when it runs, then it compares with display items written
  as literals and fails when `symbol_ops` places the symbol one pixel off.
- [ ] Given the two resolution tests, when they run, then each also holds a box or a point at 144
  as MapServer computes it, written as a literal with the source line it follows.
- [ ] Given the report test, when it runs, then it holds the boxes of a case with two priorities
  and a MultiPoint, in the cache's walk order, each edge as a literal.

## Read first

- The four tests named above; `crates/engine/src/classes.rs` (`scale_factor`); MapServer at the pin:
  `computeSymbolStyle`, `msAdjustMarkerPos` (`src/maprendering.c`), `msDrawLabelCache`
  (`src/mapdraw.c`); `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide themselves, and each is shown red against a mutation of the value it
pins; its scope is three test files, and a wrong result costs one revert; it touches none of the
parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a test that a mutation leaves green; stop: when a pinned value
disagrees with what Forager draws, the session records the difference in this file and hands off
instead of fixing the engine.
