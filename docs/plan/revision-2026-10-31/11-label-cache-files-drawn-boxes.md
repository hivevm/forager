# Ticket: File a drawn label in the cache by its box, and hold the label bound in `label`

## What to build

`insert` in `crates/label/src/cache.rs` grows each drawn label's box to the label's point before it
picks a level, so a label with a large `OFFSET` lands in the coarsest level, and every later test
visits it: with an offset near the image's size and 100,000 labels, about 5·10⁹ visits, each of
which rebuilds the label's bounds through `Members::member`.
[ADR-0032](../../adr/0032-label-cache.md)'s grid holds the box, not the box and its
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

## Taken over from `docs/plan/revision-2026-10-31b/13-label-cache-walk-handed-out.md`

Both reshape what `draw_label_cache` (`crates/label/src/cache.rs`) returns; one change carries both
(triage of 2026-11-07, on the maintainer's word).

`draw_label_cache` in `crates/label/src/cache.rs` walks the members from the highest priority down
and within a priority from the last added to the first, as `msDrawLabelCache` does, and calls its
caller back for each label it draws. The report of the labels for G-10 (`finish` in
`crates/engine/src/labels.rs`, since #137) writes that walk a second time to list every member,
drawn or discarded, in the same order. Should the cache's order change, the report diverges
silently, and the G-10 comparison pairs labels in the cache's order among those of one key. Let the
cache hand its caller every member it visits with whether it was drawn, and the report follow that
instead of its own loop. The open ticket
`docs/plan/revision-2026-10-31/11-label-cache-files-drawn-boxes.md` also changes what the cache
returns; whichever lands second takes the other's shape.

Its criteria, now this ticket's as well:

- [ ] Given a cache of labels of three priorities, some discarded, when it is drawn, then the
  members the caller is handed and their order are those of `msDrawLabelCache`'s walk, as
  literals.
- [ ] Given `crates/engine/src/labels.rs`, when it is read, then it has no loop over the priorities
  of its own.
- [ ] Given `crates/label/tests/cache_pairwise.rs` and the harness's label comparison, when they
  run, then they pass unchanged.

## Read first

- [ADR-0032](../../adr/0032-label-cache.md); `crates/label/src/cache.rs`;
  `crates/label/tests/cache_bounds.rs`; the bound in `crates/engine/src/labels.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is the label bound of Q-9 and the grid ADR-0032 decides, which ADR-0006 keeps
with the maintainer.

## Maintainer's decision (2026-11-10)

As described: a drawn label is filed by its box, its point held apart for the `MINDISTANCE` test,
its bounds kept; the cache takes the first `MAX_LABELS` and returns how many it left out, and the
engine words the diagnostic.
