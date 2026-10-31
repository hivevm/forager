# Ticket: Hold what a label's style bound to a feature draws with, not a whole style

## What to build

A label whose styles vary by feature keeps, for each feature it caches, its styles bound to the
feature as whole `Style` values (`Labels::label_styles` in `crates/engine/src/labels.rs`). A
`Style` is 624 bytes before what it points to (measured on 2026-10-31 with `size_of`), while
`crates/engine/tests/label_memory.rs` holds a cached label under 400 bytes; its maps give every
style a fixed colour, so the case that holds a bound style is not measured. Twice in the spec
`labels-2` a label's styles overran that bound, a clone in #155 and a full box in #156, each
found only once a styled case was added. Add the case of a style whose `COLOR` is bound to a
property, and hold for each feature only the values bound to it, so that the case keeps under the
bound.

## Acceptance criteria

- [ ] Given `label_memory.rs` with a `labelpoly` and a `labelpnt` style whose `COLOR` is bound to
  a property of each feature, when it runs, then each case holds a label under 400 bytes, and the
  case fails against today's code.
- [ ] Given the engine's tests of a label's styles bound to a feature, when they run, then each
  draws as before.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; `docs/CONVENTIONS.md` (what a render holds per label is measured
  where it starts to); `bindLabel`'s styles in `src/maputil.c` at the pin;
  `crates/engine/tests/label_memory.rs`; `classes::Selector::bind_label_style`.

## Blocked by

- docs/plan/revision-2026-10-31c/01-label-styles-module.md

## Mode

unattended: the memory test decides it, with a case added under the bound it already holds; the
scope is what the engine's labels hold for a feature, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no port, public interface, or style
document format, and none of the resource limits, which are Q-9's limits a diagnostic reports; the
400 bytes of the test is neither one of them nor moved here, and the ticket keeps the engine under
it rather than setting it (whether it should be a limit is ticket 06's question, with the human).
Its branch counts against ADR-0007's cap of three open agent branches. A wrong result shows as a
styled test that draws otherwise, and is undone by the revert; the new case failing against
today's code and passing after proves it right. Stop: if the bound cannot be kept without a change
to the style IR, if the 400 bytes would have to move, or after three attempts, record the
measurements here and hand off.
