# Ticket: Pass a layer's scale factor and resolution as one value

## What to build

A layer's scale factor and its resolution factor reach `symbolize` as a tuple at
`crates/symbolize/src/line.rs:195,210`, `line_marker.rs:192`, and in `label_styles.rs`, and as two
arguments at `line.rs:85,250`, `hatch.rs:65,295`, `shade.rs:126`, and `symbol::pixmap_style`. PR
#172 named it for after the normalization. `#[allow(clippy::too_many_arguments)]` stands at
`crates/engine/src/labels.rs:980` (`draw_label`) and `crates/engine/src/label_styles.rs:440`
(`Styled::draw`), and `labels.rs:404` (left to
`docs/plan/revision-2026-11-11/02-label-layout-without-mode-flag.md`). Give the two factors one
type, and the drawing of a label a context in place of its long parameter lists. No behaviour
changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the factors are one type, then it passes unchanged.
- [ ] Given the workspace, when `rg 'allow\(clippy::too_many_arguments\)' crates` runs, then it
  finds none.

## Read first

- Touches: `crates/symbolize/src/`, `crates/engine/src/labels.rs`,
  `crates/engine/src/label_styles.rs`, `crates/engine/src/classes.rs`
- `docs/backlog/label-styles-symbolized-in-symbolize.md`, which would move the drawing of a label's
  styles; a session that finds it taken up as a ticket stops and asks.

## Blocked by

- docs/plan/revision-2026-11-11/02-label-layout-without-mode-flag.md

## Mode

unattended: an interface-preserving refactoring decided by the unchanged suite; one revert;
nothing ADR-0006 keeps with the human. Stop when a test would have to change.
