# Ticket: Let the label cache hand its walk, drawn or not, to its caller

## What to build

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

## Acceptance criteria

- [ ] Given a cache of labels of three priorities, some discarded, when it is drawn, then the
  members the caller is handed and their order are those of `msDrawLabelCache`'s walk, as
  literals.
- [ ] Given `crates/engine/src/labels.rs`, when it is read, then it has no loop over the priorities
  of its own.
- [ ] Given `crates/label/tests/cache_pairwise.rs` and the harness's label comparison, when they
  run, then they pass unchanged.

## Read first

- `crates/label/src/cache.rs`, `crates/engine/src/labels.rs` (`finish`); `cache_pairwise.rs`;
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md),
  [ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md); `msDrawLabelCache` in
  MapServer's `src/mapdraw.c` at the pin.

## Blocked by

- None (can start immediately)

## Mode

unattended: the label crate's tests, the engine's, and the harness decide its criteria; an
interface change between two core crates with no host-facing part, undone by one revert; it touches
none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against
its cap of three; a wrong result shows as a moved label count or comparison; stop: when ticket 11 of
the revision of 2026-10-31 is in progress on another branch, the session waits for it instead of
cutting the same interface twice.
