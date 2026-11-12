# Ticket: Hold a label's FORCE once in the label cache

## What to build

`Member.force` (`crates/label/src/cache.rs:58-60`) is read only for a member without text, and a
member with text is forced by its `PointLabel.force` (`crates/label/src/place.rs:198`). The engine
sets both to the same value (`crates/engine/src/labels.rs` near lines 573, 580, 1115, 1117), and
the types let them disagree. PR #172 named the duplicate. Hold `FORCE` once, on the member, and let
placement read it there.

## Acceptance criteria

- [ ] Given a forced label with text and a forced `labelpoint` marker without text, when the cache
  places them, then the outcomes equal those at the base.
- [ ] Given `crates/label`, when it is read, then a label's `FORCE` is one field.

## Read first

- Touches: `crates/label/src/cache.rs`, `crates/label/src/place.rs`, `crates/engine/src/labels.rs`
- [ADR-0032](../../adr/0032-label-cache.md).
- Overlap: `docs/plan/revision-2026-10-31/11-label-cache-files-drawn-boxes.md` reshapes
  `cache.rs` too; whichever lands second takes the other's shape. Tickets 02 and 06 of this
  directory edit `crates/engine/src/labels.rs` too; work 05 apart from them in time.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests of `label` and `engine` decide it; one revert; nothing ADR-0006 keeps with
the human. Stop when a test of placement would have to change, and hand off.
