# Ticket: Let the render fuzz target reach pixmap symbols

## What to build

`fuzz/fuzz_targets/render.rs` has no pixmap symbol, and its loader returns a font, so `tiled_fill`,
`repeat`, `built_tile`, and the resampling of a pixmap get no fuzzing, though
[ADR-0015](../../adr/0015-agg-rust-rasterizer.md) names the renderer as a fuzz target. The gap
predates pixmap-symbols/09 for point pixmaps. The review of that ticket ran 4,000 seeded
combinations of NaN, infinities, and extremes on SIZE, ANGLE, GAP, MINSIZE, MAXSIZE, and OFFSET
without a panic, which is not fuzzing.

Give the target a pixmap symbol, and a loader that returns a small valid PNG, at a point, along a
line, and as a polygon's fill.

## Acceptance criteria

- [ ] Given the target, when it runs over its seeds, then coverage reaches the tiled fill and the
  resampling of a pixmap.
- [ ] Given `scripts/fuzz.sh`'s command for the target, when it runs for its full time, then it ends
  without a crash, panic, or hang.

## Read first

- `fuzz/fuzz_targets/render.rs`, `fuzz/fuzz_targets/pixmap.rs`, `scripts/fuzz.sh`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the run decides the criteria, and one revert undoes it. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human. Stop: a crash found is recorded here and
handed off, as pixmap-symbols/12 did.
