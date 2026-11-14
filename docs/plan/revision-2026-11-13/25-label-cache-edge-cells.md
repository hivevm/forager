# Ticket: Let the label cache's grid hold labels beyond the image

## What to build

From `docs/backlog/label-cache-edge-cells.md`. The grid of the label cache (`Grid` in
`crates/label/src/cache.rs`, `Grid::place`) holds a box's coordinates past the image at the image's
edge, so labels moved off the image, by a large `OFFSET` or points outside the extent's image, are
filed in the cells along the edge, and a test near the edge visits all of them. Measured: 100,000
labels offset 3,000 px right and down on a 4,096 px image, `PARTIALS TRUE`, ask for thousands of
boxes per label, against fewer than 12 for labels within the image. The maintainer chose on
2026-11-13 to change it by an ADR superseding ADR-0032's grid. Implement that ADR.

## Acceptance criteria

- [ ] Given the case above, when the cache walks it, then each label visits fewer than 12 boxes, the
  bound of `crates/label/tests/cache_bounds.rs`'s case of labels within the image.
- [ ] Given that test's memory bound per label, when the case above runs, then it holds.
- [ ] Given `crates/label/tests/cache_pairwise.rs`, when it runs unchanged, then it passes.

## Read first

- Touches: `crates/label/src/cache.rs`, `crates/label/tests/cache_bounds.rs`,
  `crates/label/tests/cache_pairwise.rs`
- The ADR that supersedes `docs/adr/0032-label-cache.md`, once the maintainer has reviewed it.

## Blocked by

- docs/plan/revision-2026-11-13/26-label-force-held-once.md

## Mode

unattended once the maintainer has reviewed the ADR's commit: the three criteria are tests; one
revert. Stop when the pairwise test's outcome moves.
