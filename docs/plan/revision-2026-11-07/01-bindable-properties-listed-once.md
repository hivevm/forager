# Ticket: List a style's bindable properties once, for binding, `varies`, and a label's held styles

## What to build

The properties of a style a feature may bind are written out field by field in several places of
`crates/engine`, each a `match` or a chain of its own: `classes::varies` (whether a style varies
by feature), `Selector::bind` (binding a style to a feature), `label_styles::Field::{of, value,
set}` (the values a cached label holds of its bound styles), and partly again in
`LabelStyles::bound_marker` (the values a bound `labelpnt` marker is computed from). A label's
style goes the whole round: bound into a `Style`, broken into `Field` values to be held, and set
back into a `Style` when drawn. A property made bindable must be added to every one of them in
step, and nothing fails when one is missed: the label would draw its written value.

Write the list of bindable properties once, with how each is read from, bound into, and set on a
`Style`, and let the four sites walk it. This extends
`docs/plan/revision-2026-10-31/10-label-binding-stage.md`, which moves the binding of a label's own
properties (`bindLabel`), not of its styles' fields; sequence the two, whichever lands first.

## Acceptance criteria

- [ ] Given a style with each bindable property bound to a feature property, when it is bound,
  tested by `varies`, and held and drawn as a label's style, then every property takes the bound
  value, each with a test that states it.
- [ ] Given a property added to the list, when the code is read, then no other place names it field
  by field.
- [ ] Given the engine's tests and `crates/engine/tests/label_memory.rs`, when they run, then they
  pass unchanged.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0028](../../adr/0028-text-stack.md); `crates/engine/src/classes.rs` (`varies`,
  `Selector::bind`), `crates/engine/src/label_styles.rs` (`Field`, `bound_marker`);
  `docs/plan/revision-2026-10-31/10-label-binding-stage.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests and the new per-property test decide it; its scope is two modules
of `engine`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a bound value drawn at its written one, which the per-property test
fails; stop: when the list cannot carry a property without a flag of its own, or the memory test
moves, the session records why here and hands off.
