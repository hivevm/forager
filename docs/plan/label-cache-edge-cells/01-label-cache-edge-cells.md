# Ticket: Labels drawn beyond the image do not pile into the label cache's edge cells

## What to build

Found while working `revision-2026-10-31/11-label-cache-files-drawn-boxes`. The grid of the label
cache (`Grid` in `crates/label/src/cache.rs`, [ADR-0032](../../adr/0032-label-cache.md)) holds a
box's coordinates past the image at the image's edge. A label that may be partial is drawn
wherever it lands, however far outside, so labels moved off the image, by a large `OFFSET` or
points outside the extent's image, are filed in the cells along the edge. A test near the edge
then visits all of them. Measured: 100,000 labels at points spread over a 4,096 px image, offset
3,000 px right and down, `PARTIALS TRUE`, ask for about 10,000 boxes per label, against fewer than
10 for labels within the image.

Let the grid hold boxes past the image in cells of their own, so that a test visits only the boxes
near it there too, each box still held once. The outcome stays MapServer's pairwise test.

## Acceptance criteria

- [ ] Given 100,000 labels with an offset of 3,000 px on a 4,096 px image, `PARTIALS TRUE`, when
      the cache runs under the visit counter of `crates/label/tests/cache_bounds.rs`, then the
      boxes asked for per label stay within the bound of that test's other cases.
- [ ] Given the memory that test measures, then it stays within its bound per label.
- [ ] Given `crates/label/tests/cache_pairwise.rs`, when it runs, then it passes unchanged.

## Read first

- [ADR-0032](../../adr/0032-label-cache.md) (the grid); `crates/label/src/cache.rs` (`Grid`);
  `crates/label/tests/cache_bounds.rs`, `crates/label/tests/cache_pairwise.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the grid ADR-0032 decides, which ADR-0006 keeps with the maintainer.
