# Where a label's styles are symbolized is undecided, and their drawing is written twice

Observed: 67b3e39 moved the symbolization of a style into crates/symbolize, but a label's styles
are still turned into drawing operations in crates/engine/src/label_styles.rs:

- `draw_label_poly` and `draw_label_marker` call symbolize's `shade` and `symbol_ops` and push the
  operations as display items themselves; that loop of operations into items with a feature's id
  is written four times (crates/engine/src/lib.rs, twice in label_styles.rs, labels.rs).
- `kind` and `marker_drawn` restate which marks can be drawn, which symbolize decides with
  `can_draw` and `mark`; a pixmap became one of them with dde6323, and the label's copy had to
  follow by hand.
- `symbolize::marker_size_with` takes a style and overrides of that style's own `SIZE` and symbol,
  for label_styles.rs alone: a second way of binding a style to a feature beside the bound
  `Style` that `draw_ops` takes.

Whether a label's `labelpoly`, `labelpnt`, and `labelcenter` styles belong to symbolize, which
would then take a label's box or point as its geometry, or stay in the engine as the label cache's
own drawing, is a decision on the boundary between the two crates that ADR-0008's table of crates
decides only by their edges. It is drafted with the maintainer, as a proposed ADR if it binds the
boundary; the code then moves to the side chosen, with one loop of operations into items and one
answer to what can be drawn, the pixels of today's tests unchanged.

Where: crates/engine/src/label_styles.rs, crates/engine/src/labels.rs, crates/engine/src/lib.rs,
crates/symbolize/src/draw.rs, crates/symbolize/src/symbol.rs (`marker_size_with`); ADR-0008,
ADR-0029, ADR-0031; docs/plan/revision-2026-11-07/01-bindable-properties-listed-once.md.
