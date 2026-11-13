# Ticket: Decide where a label's styles are symbolized, and draw them there

## What to build

1eec7a6 moved the symbolization of a style into `crates/symbolize`, but a label's styles are still
turned into drawing operations in `crates/engine/src/label_styles.rs`:

- `draw_label_poly` and `draw_label_marker` call `symbolize`'s `shade` and `symbol_ops` and push
  the operations as display items themselves; that loop of operations into items with a feature's
  id is written four times, twice in `label_styles.rs` and twice in `lib.rs` (`draw_points` and
  the closure in `draw`), and `labels.rs`'s `draw_label` builds a fifth item with a feature's id
  for a label's glyphs.
- `kind`, with `shade_drawn` and `marker_drawn`, restates which marks can be drawn, which
  `symbolize` decides with `can_draw` and `mark`. `mark` has drawn a pixmap since 1eec7a6; the
  label's copy did not follow, so a `labelpnt` or `labelcenter` style of a pixmap is refused today
  ("no vector or ellipse of the symbol set"), and `label_marker` sizes its marker without the
  render's pixmaps. One answer to what can be drawn has to settle whether that refusal stays the
  label's own or the pixmap is drawn; the draft names which.
- `symbolize::marker_size_with` takes a style and overrides of that style's own `SIZE` and symbol,
  outside `symbolize` for `label_styles.rs` alone (`marker_of`): a second way of binding a style
  to a feature beside the bound `Style` that `draw_ops` takes.

Whether a label's `labelpoly`, `labelpnt`, and `labelcenter` styles belong to `symbolize`, which
would then take a label's box or point as its geometry, or stay in the engine as the label cache's
own drawing, is a decision on the boundary between the two crates that ADR-0008's table of crates
decides only by their edges. Draft it with the maintainer, a `proposed` ADR if it binds the
boundary, and then move the code to the side chosen, with one loop of operations into items and
one answer to what can be drawn. `symbolize`'s functions are generic over a style's transform
since b0d30ce, so a label's `Style<LabelTransform>` reaches them as it is.

The move may also break the cycle among the engine's modules that
`docs/backlog/engine-modules-import-each-other.md` records: `label_styles` calls
`crate::labels::push` and `crate::labels::make_room`.

## Acceptance criteria

- [ ] Given the draft, when the maintainer reads it, then it names where a label's styles are
  symbolized and why, in an ADR-only pull request if it is an ADR, and whether a `labelpnt` or
  `labelcenter` style of a pixmap is drawn or refused.
- [ ] Given a label with a `labelpoly`, a `labelpnt`, and a `labelcenter` style, when it is drawn
  after the move, then the pixels are those of today's tests (`crates/engine/tests/labels.rs`),
  and a `labelpnt` of a pixmap is refused as today or, where the draft draws it, drawn as a point
  layer's pixmap marker is, by a test of its pixels.
- [ ] Given the code, when it is read, then the decision whether a mark can be drawn and the loop
  of operations into items are each written once.

## Read first

- Touches: `crates/engine/src/label_styles.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/lib.rs`, `crates/symbolize/src/draw.rs`, `crates/symbolize/src/shade.rs`,
  `crates/symbolize/src/symbol.rs` (`mark`, `marker_size_with`).
- [ADR-0008](../../adr/0008-ports-and-adapters-crates.md),
  [ADR-0031](../../adr/0031-label-glyphs.md),
  [ADR-0029](../../adr/0029-pixmaps-held-once-per-render.md); the `engine` and `symbolize`
  bullets of [`docs/ARCHITECTURE.md`](../../ARCHITECTURE.md);
  `crates/arch-test/tests/symbolize_modules.rs`, which holds the direction of `symbolize`'s
  modules; `crates/engine/src/bindable.rs`, where a style's bindable properties are listed once
  (28b62a8).

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides the boundary between `engine` and `symbolize`, which ADR-0008's table
of crates decides only by their edges.
