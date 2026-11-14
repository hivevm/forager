# Ticket: Narrow what drawing a label's styles takes

## What to build

`Styled::draw` (`crates/engine/src/label_styles.rs:412-428`) takes a `Placed`
(`label_styles.rs:339-344`) that holds a whole cache `Member` and its `Placement`, and reads only
the member's point, its label, and the placement's anchor. So `draw_at_once`
(`crates/engine/src/labels.rs:653-701`) builds a zero `TextBox` and a `Member` with
`force: true`, `partials: true`, and `min_distance: -1.0` to satisfy the type. Beside it:

- `Styled::draw` checks `label_style_refused` per style (`label_styles.rs:432`), which cannot
  happen there: `fixed()` refuses the whole label when any style is refused
  (`crates/engine/src/label_binding.rs:265-269`).
- The lookup `map.layers[a].classes[b].labels[c]` of a `LabelAt` tuple is written eight times
  (`labels.rs:450,486,541,571,867,1123,1207`, `label_styles.rs:413`).

Let `Styled::draw` take only what it reads, drop the check that cannot fire, and give `LabelAt` a
method that finds its label. No behaviour changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the interface is narrowed, then it passes with every
  expected value unchanged.
- [ ] Given `draw_at_once`, when the change lands, then it builds no `Member` and no `TextBox`.

## Read first

- Touches: `crates/engine/src/label_styles.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/label_binding.rs`
- `docs/backlog/engine-modules-import-each-other.md`: `LabelAt`'s method may add an edge to that
  cycle; place it so it adds none.

## Blocked by

- docs/plan/revision-2026-11-13/07-marker-bounds-in-one-place.md

## Mode

unattended: a refactoring decided by the suite's expected values; one revert. Stop when a test's
expected value moves.
