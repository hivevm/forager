# Ticket: Split the binding of a label out of `labels.rs` and share it with the styles'

## What to build

About 300 of the 1,564 lines of `crates/engine/src/labels.rs` are MapServer's `bindLabel` (`Prop`,
`Fixed`, `BoundLabel`, `bind`, `bound_integer`, `bound_position`, `bound_align`, `prop`, `fixed`,
`int_cast`), which does what `Binder` in `crates/engine/src/classes.rs` does for `bindStyle`, with
defaults, number reading, and colour reading of its own. Move it into a module of its own sharing
one port of MapServer's `bind*Attribute` with `classes.rs`, and pass `Labels::collect` what it
uses of the `Selector`, not the whole.

The ticket this one waited on, `label-binding-state/01`, was dropped by the triage of 2026-11-07
(bd2ff53). Two open tickets rewrite the same code:
`docs/plan/revision-2026-11-07/01-bindable-properties-listed-once.md` lists a style's bindable
properties once, beside this binding of a label's own, and
`docs/plan/revision-2026-11-07/02-labelcache-off-path-of-its-own.md` reworks `collect` and
`feature`. Work the three one after another, not in parallel; whichever lands later rebases onto
the others.

## Acceptance criteria

- [ ] Given the label tests of `crates/engine/tests/labels.rs`, when they run after the split, then
  they pass unchanged.
- [ ] Given a label and a style bound to the same property holding `" 12abc"`, when each is bound,
  then both read it through the one shared function.

## Read first

- `crates/engine/src/labels.rs`; `Binder` and `Selector` in `crates/engine/src/classes.rs`.
- `docs/plan/revision-2026-11-07/01-bindable-properties-listed-once.md` and
  `docs/plan/revision-2026-11-07/02-labelcache-off-path-of-its-own.md`, for what they change in
  the same code.

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's label and class tests decide its criteria; its scope is the binding code
of `crates/engine`, and a wrong result costs one revert; it touches none of the parts
[ADR-0006](../../adr/0006-harness.md) keeps with the maintainer and counts against its cap of three
open agent branches; a wrong result shows as a failing label test. Do not start it while
`revision-2026-11-07/01` or `/02` is claimed. Stop: after two attempts that leave
`scripts/check-all.sh` red, or when a change of `revision-2026-11-07/01` or `/02` that landed
conflicts with the split, leave the state in this file and hand off.
