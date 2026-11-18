# The label canvas reads a spent marker budget again on every take, across layers

**Observed:** the design revision of 2026-11-18 (2), from the review of ticket
revision-2026-11-16/09 (merge 92f5e5d0, *Could not confirm*). It was first filed as a ticket, then
judged untestable at a real seam.

`Canvas::take_refusals` (`crates/symbolize/src/canvas.rs`) reads `markers_spent` and
`trees_over_limit` again on every call, because the layer's budgets hold them. Its doc and
`the_canvas_yields_its_markers_spent_with_its_refusals_and_holds_on_to_them` (`Verifies: Q-9`)
decide that on purpose. The engine draws every label's styles on one shared label canvas
(`crates/engine/src/label_styles.rs`) and takes it once per label. It files each take under the
label's own layer (`Labels::refused`, `crates/engine/src/labels.rs`). If that canvas's budget were
ever spent, every later take would report it again, on whichever layer's label came next.

The budget is never spent today. The label path never takes from it, since `label_marker` goes
through `marker_ops`, and the label canvas has no style trees. So no render shows the carry-over,
and a test can show it only by a seam that spends the label canvas's budget by hand. No test pins
that the label path never takes from the budget.

**Where:** `crates/engine/src/label_styles.rs` (the label canvas, `take_refusals`),
`crates/engine/src/labels.rs` (`Labels::refused`, `layer_refusals`),
`crates/symbolize/src/canvas.rs`; Q-9.

**What a change weighs:**

- **Pin the fact**: a test holds the label path to taking nothing from the marker budget. Then a
  change that makes it take fails there, before it can carry a report to another layer.
- **Give the label canvas no layer state**: its own budget per layer, or `Labels::refused` takes
  only the per-take fields. That is more code for a path no input reaches.
- **Leave it**: no work while the label path spends no markers.
