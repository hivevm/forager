# Ticket: Bind a labelpoint marker's style to a feature one way

## What to build

From `docs/backlog/marker-size-bound-twice.md`. `symbolize::marker_size_with` takes a style with its
`SIZE` and symbol given apart, in place of the style's own. Only `label_style_marker` in
`crates/symbolize/src/label.rs` calls it, for the engine's `crates/engine/src/label_styles.rs`: the
label cache asks `LabelStyles::bound_marker` for the marker of a label whose styles vary by feature
each time it tests the label. Every other style reaches `symbolize` bound, as the `Style` that
`draw_ops` and `marker_size_of` take. Give `marker_size_of` the bound style, built once per test as
`LabelStyles::style` builds it to draw it, and drop `marker_size_with`. Measure first: the release
build of `crates/engine/tests/labels.rs`'s case of 100,000 labels before and after.

## Acceptance criteria

- [ ] Given `rg -n 'fn marker_size_with' crates`, when the change lands, then it finds none.
- [ ] Given the suite at the base, when the style is bound one way, then it passes with every
  expected value unchanged, the memory bounds of Q-9 among them.
- [ ] Given the measurement, when the hand-off is written, then it states both times.

## Read first

- Touches: `crates/symbolize/src/symbol.rs`, `crates/symbolize/src/label.rs`,
  `crates/engine/src/label_styles.rs`
- `docs/SPECIFICATION.md`, Q-9's memory bounds.

## Blocked by

- docs/plan/revision-2026-11-13/07-marker-bounds-in-one-place.md

## Mode

unattended: the suite's expected values and the Q-9 memory tests decide it; one revert. Stop when
the measured case is slower by more than a tenth, or a memory bound moves; hand off with the
numbers.
