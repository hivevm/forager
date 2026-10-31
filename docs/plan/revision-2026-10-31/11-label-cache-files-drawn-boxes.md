# Ticket: File a drawn label in the cache by its box, and hold the label bound in `label`

## What to build

`insert` in `crates/label/src/cache.rs` grows each drawn label's box to the label's point before it
picks a level, so a label with a large `OFFSET` lands in the coarsest level, and every later test
visits it: with an offset near the image's size and 100,000 labels, about 5·10⁹ visits, each of
which rebuilds the label's bounds through `Members::member`.
[ADR-0035](../../adr/0035-label-cache-grid-and-bound.md)'s grid holds the box, not the box and its
point. Hold the point for the `MINDISTANCE` test apart from the box, and keep each drawn label's
bounds instead of rebuilding them at each visit. In the same file, `draw_label_cache` only documents
that its caller holds at most `MAX_LABELS`; the cut and the warning are in the engine, so another
`Members` passes it any count. Let the cache take the first `MAX_LABELS` and return how many it left
out, and the engine word the diagnostic.

## Acceptance criteria

- [ ] Given 100,000 labels with an offset of 3,000 px on a 4,096 px image, when the cache runs under
  the visit counter of `crates/label/tests/cache_bounds.rs`, then the visits per label stay within
  the bound the other cases of that test hold.
- [ ] Given a `Members` of 100,001 labels, when `draw_label_cache` runs, then it reports one left
  out without the engine.

## Read first

- [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md); `crates/label/src/cache.rs`;
  `crates/label/tests/cache_bounds.rs`; the bound in `crates/engine/src/labels.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is the label bound of Q-9 and the grid ADR-0035 decides, which ADR-0007 keeps
with the maintainer.
