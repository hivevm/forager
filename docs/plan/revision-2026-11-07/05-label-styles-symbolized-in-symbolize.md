# Ticket: Decide where a label's styles are symbolized, and draw them there

## What to build

67b3e39 moved the symbolization of a style into `crates/symbolize`, but a label's styles are still
turned into drawing operations in `crates/engine/src/label_styles.rs`:

- `draw_label_poly` and `draw_label_marker` call `symbolize`'s `shade` and `symbol_ops` and push
  the operations as display items themselves; that loop of operations into items with a feature's
  id is written four times (`lib.rs`, twice in `label_styles.rs`, `labels.rs`).
- `kind` and `marker_drawn` restate which marks can be drawn, which `symbolize` decides with
  `can_draw` and `mark`; a pixmap became one of them with dde6323, and the label's copy has to
  follow by hand.
- `symbolize::marker_size_with` takes a style and overrides of that style's own `SIZE` and symbol,
  for `label_styles.rs` alone: a second way of binding a style to a feature beside the bound
  `Style` that `draw_ops` takes.

Whether a label's `labelpoly`, `labelpnt`, and `labelcenter` styles belong to `symbolize`, which
would then take a label's box or point as its geometry, or stay in the engine as the label cache's
own drawing, is a decision on the boundary between the two crates. Draft it with the maintainer, a
`proposed` ADR if it binds the boundary, and then move the code to the side chosen, with one
loop of operations into items and one answer to what can be drawn.

## Acceptance criteria

- [ ] Given the draft, when the maintainer reads it, then it names where a label's styles are
  symbolized and why, in an ADR-only pull request if it is an ADR.
- [ ] Given a label with a `labelpoly`, a `labelpnt` of a pixmap, and a `labelcenter` style, when it
  is drawn after the move, then the pixels are those of today's tests.
- [ ] Given the code, when it is read, then the decision whether a mark can be drawn and the loop
  of operations into items are each written once.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0028](../../adr/0028-text-stack.md),
  [ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md);
  `crates/engine/src/label_styles.rs`, `crates/symbolize/src/draw.rs`,
  `crates/symbolize/src/symbol.rs` (`marker_size_with`);
  `docs/plan/revision-2026-11-07/01-bindable-properties-listed-once.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides the boundary between `engine` and `symbolize`, which ADR-0009's table
of crates decides only by their edges.
