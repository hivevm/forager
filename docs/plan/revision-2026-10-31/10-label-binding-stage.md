# Ticket: Split the binding of a label out of `labels.rs` and share it with the styles'

## What to build

About 400 of the 1,002 lines of `crates/engine/src/labels.rs` are MapServer's `bindLabel` (`Prop`,
`Fixed`, `BoundLabel`, `bind`, `bound_integer`, `bound_position`, `bound_align`), which does what
`classes::Binder` does for `bindStyle` with defaults, number reading, and colour reading of its own.
Move it into a module of its own sharing one port of MapServer's `bind*Attribute` with `classes.rs`,
and pass `collect` what it uses of the `Selector`, not the whole.

## Acceptance criteria

- [ ] Given the label tests of `crates/engine/tests/labels.rs`, when they run after the split, then
  they pass unchanged.
- [ ] Given a label and a style bound to the same property holding `" 12abc"`, when each is bound,
  then both read it through the one shared function.

## Read first

- `crates/engine/src/labels.rs`; `Binder` in `crates/engine/src/classes.rs`; the ticket it waits on.

## Blocked by

- docs/plan/label-binding-state/01-label-binding-state.md

## Mode

unattended: the engine's label and class tests decides its criteria; its scope is the binding code
of `crates/engine`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a failing label test; stop: after two attempts that leave
`scripts/check-all.sh` red, or when the label-binding-state ticket's change conflicts with the
split, the session leaves its state in this file and hands off.
