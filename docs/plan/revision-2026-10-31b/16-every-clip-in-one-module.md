# Ticket: Hold every clip to the image in one module of `render` (ADR-0039)

## What to build

#131 moved the one segment clip and the margin past the image into `crates/render/src/clip.rs`, so
that the hatch's bound and `render-agg` measure and clip alike. To do so `forager_render::clip`
publishes `CLIP_MARGIN`, `drawn_width`, `Rect`, `Axis`, `crossing`, and `segment`; `Axis` and
`crossing` have one user outside `render`, the Sutherland–Hodgman ring clip that stays in
`crates/render-agg/src/clip.rs` with `clip_line`, beside an `Edge` enum of its own that re-encodes
the `(Axis, value)` edges `segment` already uses. The image grown by the margin is written four
ways: `Rect::around_image` (render-agg's `lib.rs`), `Hatch::line_rect` (`render/src/hatch.rs`), a
`clip_box(-CLIP_MARGIN, …)` built by hand in `render-agg/src/glyphs.rs`, and the edges of
`Edge`. The clip module is reached three ways (`crate::clip::Rect` re-exported,
`forager_render::clip::…`, `crate::CLIP_MARGIN`). agg-rust's fixed-point limits live in two files of
`render`: the margin in `clip.rs`, `MAX_GLYPH_REACH` in `lib.rs`. #131 said so in its third doubt:
backend clipping details are now part of the display-list crate.

[ADR-0009](../../adr/0009-ports-and-adapters-crates.md) gives `render` the display list and the
renderer trait; [ADR-0015](../../adr/0015-agg-rust-rasterizer.md) has `render-agg` clip every path
before agg-rust; neither says where the clip's code lives.
[ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md), proposed, puts the
whole clip, segments, lines, and rings, and the image grown by its margin in `render::clip`, with
`Axis`, `crossing`, and the edges private, and asks the maintainer where agg-rust's limits stand.
Implement it while it is proposed, after the maintainer's review: one module clips, one value
grows the image, and the open ticket
`docs/plan/revision-2026-10-31/12-one-glyph-reach.md` puts its predicate beside it.

## Acceptance criteria

- [ ] Given ADR-0039 as reviewed, when the module is read, then every clip to the image and
  the image grown by its margin are written once, and `render-agg` reaches them one way.
- [ ] Given the renderer's tests, `crates/engine/tests/render.rs`, and the harness, when they run,
  then they pass unchanged and no reference image is re-blessed.
- [ ] Given `crates/arch-test`, when it runs, then the edges of ADR-0009 hold.

## Read first

- ADR-0039, ADR-0009, ADR-0015, [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md);
  `crates/render/src/clip.rs`, `hatch.rs`, `lib.rs`; `crates/render-agg/src/clip.rs`, `glyphs.rs`,
  `lib.rs`; #131's doubts (`gh pr view 131`).

## Blocked by

- None (can start immediately)

## Mode

with the human: ADR-0039 decides a crate's responsibility that ADR-0009 and ADR-0015 leave open,
and it moves code of the hatch's resource bound (Q-9), which
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer.
