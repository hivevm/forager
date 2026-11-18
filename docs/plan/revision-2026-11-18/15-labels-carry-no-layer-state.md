# Ticket: The labels carry no spent markers and no trees' bound from one layer to another

## What to build

`Canvas::take_refusals` (`crates/symbolize/src/canvas.rs`) hands on what a drawing refused since
it was last asked: the hatches, pixmaps, symbols, and glyphs. Two fields of the same `Refusals`,
`markers_spent` and `trees_over_limit`, are layer state. Each call reads them again, because the
layer's budgets hold on to them. That is deliberate:

- the doc of `take_refusals` says so (`canvas.rs:209-214`);
- `the_canvas_yields_its_markers_spent_with_its_refusals_and_holds_on_to_them`
  (`crates/symbolize/src/canvas/tests.rs`, `Verifies: Q-9`) pins it. It asserts that a second
  take still reports the spent markers. A layer's later features stay refused, and so does its
  report.

The engine takes a layer canvas once per layer, so a layer reports each once. The label canvas is
different. The labels take it once per label and merge what it yields into the label's layer
(`Labels::refused`, `crates/engine/src/labels.rs`). Ticket 09 of revision-2026-11-16 folded both
fields into `Refusals` (merge 92f5e5d0). Its review named what keeps this safe: the label path
never takes from the marker budget, because `label_marker` goes through `marker_ops`, and it has
no style trees. No test pins either fact. A change that makes the label path take from the budget
would report the spent markers again on every layer whose labels are drawn after.

The canvas keeps its semantics, and its test stays as it is. The labels report spent markers and a
trees' bound only on the layer whose drawing spent or passed them. A test decides this through the
engine, from a render that would show the carry-over. The session decides how by the smaller
change and says which in the hand-off. It may give the label canvas its own state per layer, take
only the per-take fields into `Labels::refused`, or use another way. The diagnostics of every
existing map stay as they are.

## Acceptance criteria

- [ ] Given the label path's refusals of one layer with markers spent and a trees' bound set, as
  a test builds them, when a later layer's labels are merged and reported, then the later layer
  reports neither.
- [ ] Given `the_canvas_yields_its_markers_spent_with_its_refusals_and_holds_on_to_them` and the
  doc of `take_refusals`, when the change is done, then both are unchanged, and Q-9's
  `Verifies:` markers are as before.
- [ ] Given every test of the workspace, when they run, then all pass, and no expected diagnostic
  changed.

## Read first

- Touches: `crates/engine/src/labels.rs` (`Labels::refused`, `layer_refusals`),
  `crates/engine/src/label_styles.rs` (the label canvas, `take_refusals`),
  `crates/symbolize/src/canvas.rs` (read, and changed only if the smaller change is there), the
  engine's tests of the labels' reports
- The merge message of 92f5e5d0, its *Could not confirm* and *Review*; Q-9; the conventions *A bound
  of Q-9 reports once* and *A test holds a value it states*.

## Blocked by

- docs/plan/revision-2026-11-18/14-engine-layer-reports-one-module.md

## Mode

unattended: a new test and the existing diagnostics tests decide it, and a wrong result costs one
revert. Stop when the only way to meet criterion 1 changes the canvas test or a Q-9 marker; hand off
with that, since it reverses a decision the canvas states.
