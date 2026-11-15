# Ticket: A test that a reprojected polygon keeps the IR's ring order

## What to build

Twice the reprojection broke an invariant of Forager's IR that MapServer does not have. Review 01
(#3) found that a polygon whose exterior ring did not reproject kept its holes, so a hole became
the exterior (fixed in d6cdd093). Review 05-07 (S2) found that a moved ring of a multipolygon
filled holes, since Forager fills each polygon apart (fixed in e0ed507f). Each was caught by a
reviewer, not by a check. A test holds the invariant for every shape `proj` reprojects in its
reference set `crates/proj/tests/reference/shapes.tsv`, and for the engine's reprojection of
polygons with holes. Each polygon left has its exterior first, and none is left with holes and
no exterior.

## Acceptance criteria

- [ ] Given every polygon of the reference shapes, when the test reprojects it through the code
  that ships, then each polygon left starts with its exterior and no hole outlives its exterior.
- [ ] Given the fault of review 01 #3 put back by hand, when the test runs, then it fails. Say in
  the hand-off that this was tried.

## Read first

- Touches: `crates/proj/tests/reference.rs` or `crates/engine/tests/reproject.rs`,
  `crates/engine/src/reproject.rs`, `crates/proj/src/shape.rs`
- `docs/adr/0037-reprojection-ported-in-proj.md`; `docs/CONVENTIONS.md`, *A test holds a value it
  states*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the test decides it; one revert. Stop when the invariant fails on the tree as it is.
That is a defect for its own ticket.
