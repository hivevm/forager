# ADR-0039: The clip to the image is one module of `render`, shared by the hatch's bound and every renderer

- **Status:** 🟡 proposed
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/render/src/clip.rs`, the clipping of paths in `crates/render-agg`, and every renderer or bound that measures or clips a path against the image

## Context

[ADR-0009](0009-ports-and-adapters-crates.md) gives `render` the display list and the renderer
trait; [ADR-0015](0015-agg-rust-rasterizer.md) has `render-agg` clip every path near the image
before agg-rust rasterizes it, since agg-rust's fixed-point coordinates overflow far out;
[ADR-0032](0032-hatch-cut-by-i-overlay.md) bounds a hatch in `render` before it is cut. Neither
says where the code that clips to the image lives.

The bound measured a hatch's lines with a clip of its own and the renderer clipped them with
another, and the two disagreed on lines with both ends far out: 400,000 lines of `(-1e150, 5)` to
`(1e150, 5)` on a 100 × 100 image passed the bound at 0 px each, while the renderer drew about
106 px each (Q-9). The design revision of 2026-10-31 filed one clip, and #131 moved the segment
clip and the margin past the image into `crates/render/src/clip.rs`. To let `render-agg` use it,
that module publishes `CLIP_MARGIN`, `drawn_width`, `Rect`, `Axis`, `crossing`, and `segment`. The
ring clip (Sutherland–Hodgman) and the line clip stay in `render-agg`, with an `Edge` enum that
re-encodes the edges `segment` uses. The image grown by the margin is written four ways, and
agg-rust's limits stand in two files of `render` (`CLIP_MARGIN` in `clip.rs`, `MAX_GLYPH_REACH` in
`lib.rs`). The forces: Q-9 asks that the bound measure what is drawn, so the measure and the clip
must be one; ADR-0009 keeps `render` free of a backend; Simplicity asks for one place.

Questions for the maintainer:

1. Does the whole clip, segments, lines, and rings, stand in `render::clip`, with `Axis`,
   `crossing`, and the edges private, as this ADR proposes; or does `render` keep only what the
   hatch's bound needs and `render-agg` the rest, with the edges shared?
2. Are the margin past the image and the reach of a glyph (`MAX_GLYPH_REACH`) properties of the
   display list, which every renderer is held to, or of agg-rust, which `render` then names only
   because the bound must measure what `render-agg` draws?
3. Does a second renderer, should one come, clip through the same module, or is the module the
   reference renderer's and the bound's alone?

## Decision

We will hold the clip of every path to the image, its segments, lines, and rings, and the image
grown by its margin in one module of `render`, `clip.rs`, which the hatch's bound measures with and
every renderer clips with, and publish from it only the clips and the rectangle.

**Out of scope:** how a renderer rasterizes what the clip leaves; the cut of a hatch from its
polygon, which ADR-0032 gives i_overlay; the clip of a hatched polygon to the class's clip buffer in
the engine, which is MapServer's `msClipPolygonRect` and not a clip to the image.

## Alternatives considered

- **The clip in `render-agg`, the bound asking the renderer** — the bound would need the renderer
  crate, against ADR-0009's edges, or a trait method for a measure that is the same for every
  renderer.
- **A clip in each crate, held equal by a test** — the state before #131, under which the two
  drifted apart unseen until the Q-9 hole above.
- **As it stands since #131** — one segment clip in `render`, the ring and line clips in
  `render-agg` on `render`'s public `Axis` and `crossing`: the clip's parts are public for one user,
  and four spellings of one rectangle remain.

## Sources / Prior art

- Liang and Barsky, "A new concept and method for line clipping", ACM Transactions on Graphics 3(1),
  1984; Sutherland and Hodgman, "Reentrant polygon clipping", Communications of the ACM 17(1), 1974.
- AGG 2.4's `rasterizer_sl_clip` and agg-rust's `clip_box`, which `render-agg` calls; #131
  (`gh pr view 131`), its third doubt.

## Consequences

- Positive: the bound and the renderer measure one geometry; one rectangle; `render-agg` reaches
  the clip one way; a glyph's reach can stand beside the margin.
- Negative / trade-offs: `render` holds code no display item needs to be built, only to be drawn
  and bounded; a renderer that clips by other means still links it.
- Follow-ups: where the reach of a glyph is decided, which the revision ticket on one glyph reach
  takes up.

## Enforcement

A test of `render` that clips segments, lines, and rings with both ends far out and holds the
hatch's measure to the length of what the clip leaves cites this ADR with `Verifies: ADR-0039`;
`crates/arch-test` keeps ADR-0009's edges. That `render-agg` clips through no code of its own is
review.
