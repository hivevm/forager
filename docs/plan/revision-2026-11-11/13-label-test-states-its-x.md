# Ticket: State the x of a LABELCACHE OFF multipoint's label in its test

## What to build

`each_point_marker_then_label` in `crates/engine/tests/labels.rs` (near line 4012) takes the first
label's x from the output under test and asserts only that the second lies 5 px further. A label
drawn at a wrong x at both points passes. `docs/CONVENTIONS.md` says a test holds a value it
states; PR #176 named this among its lesser findings. State the x the label is drawn at, from its
point, its position, and its font's advances as the test's comment derives them.

## Acceptance criteria

- [ ] Given the multipoint tests of `LABELCACHE OFF`, when they run, then each asserts the label's
  x as a stated value.
- [ ] Given the engine drawing the first label 1 px off, when the tests run, then one fails.

## Read first

- Touches: `crates/engine/tests/labels.rs`
- `docs/CONVENTIONS.md` (*A test holds a value it states, never one the code under test computes*).

## Blocked by

- None (can start immediately)

## Mode

unattended: a test-only change; one revert; nothing ADR-0006 keeps with the human. Stop if the
stated value differs from what the engine draws, and hand off: that is a defect, not this ticket.
