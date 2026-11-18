# Ticket: A canvas reports each of its refusals once, its spent markers and its trees' bound included

## What to build

`Canvas::take_refusals` (`crates/symbolize/src/canvas.rs`) hands on what a drawing left undrawn
since it was last asked: the hatches, pixmaps, symbols, and glyphs it refused. Two fields of the
same `Refusals` are not taken but read again on every call, `markers_spent` and
`trees_over_limit`, since the layer's budgets hold them. The doc says so. Ticket 09 of
revision-2026-11-16 folded both into `Refusals` (merge 92f5e5d0), and its review named the
consequence:

- the engine calls `take_refusals` once per layer canvas, so a layer reports each once today;
- the label canvas is taken once per label, and the labels merge what it yields per layer
  (`Labels::refused`, `crates/engine/src/labels.rs`). It reports no spent markers only because the
  label path never takes from the budget (`label_marker` goes through `marker_ops`), and no test
  pins that. A change that makes it take from the budget would report the spent markers again on
  every layer whose labels are drawn after.

Besides, `refusal_reports` (`crates/engine/src/layer.rs`) never reads `trees_over_limit`, which the
root takes out of `Refusals` before the layer's reports (`crates/engine/src/lib.rs`), and
`Canvas::markers_spent` is left as a method for tests only.

A second `take_refusals` on the same canvas reports nothing the first reported, the spent markers
and the trees' bound included, while the budgets keep refusing past their bound. Whether the
trees' bound stays a field of `Refusals` or is taken beside it, the session decides by the smaller
change and says which in the hand-off. The diagnostics of every map stay as they are.

## Acceptance criteria

- [ ] Given a canvas whose markers passed the budget and whose class styles' trees passed their
  bound, when `take_refusals` is called twice, then the first reports both and the second neither,
  and a marker drawn after it is still refused.
- [ ] Given the label path made to take a marker from the budget in a throwaway copy, when two
  point layers with labels render, then the spent markers are reported on the layer that spent them
  only, and the hand-off says so; the copy is not committed.
- [ ] Given every test of the workspace, when they run, then all pass, and no expected diagnostic
  changed.

## Read first

- Touches: `crates/symbolize/src/canvas.rs`, `crates/symbolize/src/canvas/tests.rs`,
  `crates/symbolize/src/line_marker.rs` (the budget), `crates/engine/src/labels.rs`
  (`Labels::refused`, `layer_refusals`), `crates/engine/src/lib.rs` (the take after each layer),
  `crates/engine/src/layer.rs` (`refusal_reports`)
- The merge message of 92f5e5d0, its *Could not confirm* and *Review*; Q-9; the conventions *A bound
  of Q-9 reports once* and *A count held to a bound of Q-9 stops at the bound*.

## Blocked by

- docs/plan/revision-2026-11-18/14-engine-layer-reports-one-module.md

## Mode

unattended: the canvas's tests and the engine's diagnostics tests decide it, and a wrong result
costs one revert. Stop when a report moves from one layer to another in any existing test; hand
off with the test and the report.
