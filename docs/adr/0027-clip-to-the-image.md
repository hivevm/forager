# ADR-0027: The clip to the image is one module of `render`, shared by the hatch's bound and every renderer

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/render/src/clip.rs`, the clipping of paths in `crates/render-agg`, and every renderer or bound that measures or clips a path against the image

## Context

[ADR-0008](0008-ports-and-adapters-crates.md) gives `render` the display list and the renderer
trait. [ADR-0025](0025-agg-rust-rasterizer.md) has `render-agg` clip every path near the image
before agg-rust rasterizes it, since agg-rust's fixed-point coordinates overflow far out.
[ADR-0028](0028-hatch-cut-by-i-overlay.md) bounds a hatch in `render` before it is cut. Neither
says where the code that clips to the image lives.

The bound once measured a hatch's lines with a clip of its own and the renderer clipped them with
another, and the two disagreed on lines with both ends far out: 400,000 lines of `(-1e150, 5)` to
`(1e150, 5)` on a 100 × 100 image passed the bound at 0 px each, while the renderer drew about
106 px each (Q-9). The design revision of 2026-10-31 filed one clip. #131 moved the segment clip
and the margin past the image into `render`, but left the ring clip (Sutherland–Hodgman) and the
line clip in `render-agg`, on parts of the clip `render` had to publish for it, and the image
grown by the margin written four ways. The forces: Q-9 asks that the bound measure what is drawn,
so the measure and the clip must be one; ADR-0008 keeps `render` free of a backend; Simplicity asks
for one place.

The maintainer settled three questions. The whole clip, segments, lines, and rings, stands in
`render::clip`, its edges private, rather than `render` keeping only what the hatch's bound needs.
The margin past the image and the reach of a glyph are properties of the display list, which every
renderer is held to, not of agg-rust alone. A second renderer, should one come, clips through the
same module.

Today `crates/render/src/clip.rs` holds the segment, line, and ring clips, the rectangle
(`Rect::around_image`), the width a stroke is drawn at (`drawn_width`), and the reach of a glyph
(`MAX_GLYPH_REACH`, `within_glyph_reach`); `render-agg` clips paths, glyphs, and hatches through it.

## Decision

We will hold the clip of every path to the image, and the image grown by its margin, in one module
of `render`, `clip.rs`, which the hatch's bound measures with and every renderer clips with.

- **Contents.** The module must hold the clips of segments, lines, and rings, and the image grown
  by its margin.
- **Public items.** The module must publish only the clips, the rectangle, the width a stroke is
  drawn at, and the reach of a glyph. Only the image grown by its margin may build the rectangle.
  Its edges must stay private.
- **Properties of the display list.** The margin past the image and the reach of a glyph
  (`MAX_GLYPH_REACH`, with its predicate `within_glyph_reach`) are properties of the display list
  and must stand in that module.
- **Every renderer.** Every renderer, not only `render-agg`, is held to the margin and the reach,
  and must clip through the module.

**Out of scope:** how a renderer rasterizes what the clip leaves; the cut of a hatch from its
polygon, which ADR-0028 gives i_overlay; the clip of a hatched polygon to the class's clip buffer in
the engine, which is MapServer's `msClipPolygonRect` and not a clip to the image.

## Alternatives considered

- **The clip in `render-agg`, the bound asking the renderer** — the bound would need the renderer
  crate, against ADR-0008's edges, or a trait method for a measure that is the same for every
  renderer.
- **A clip in each crate, held equal by a test** — the state before #131, under which the two
  drifted apart unseen until the Q-9 hole above.
- **The split of #131** — one segment clip in `render`, the ring and line clips in `render-agg` on
  `render`'s public `Axis` and `crossing`: the clip's parts public for one user, and four
  spellings of one rectangle.

## Sources / Prior art

- Liang and Barsky, "A new concept and method for line clipping", ACM Transactions on Graphics 3(1),
  1984; Sutherland and Hodgman, "Reentrant polygon clipping", Communications of the ACM 17(1), 1974.
- AGG 2.4's `rasterizer_sl_clip` and agg-rust's `clip_box`, which `render-agg` calls; #131
  (`gh pr view 131`), its third doubt.

## Consequences

- Positive: the bound and the renderer measure one geometry; one rectangle; `render-agg` reaches
  the clip one way; a glyph's reach stands beside the margin.
- Negative / trade-offs: `render` holds code no display item needs to be built, only to be drawn
  and bounded; a renderer that clips by other means still links it.
- Follow-ups: none.

## Enforcement

`crates/render/tests/clip.rs` clips segments, lines, and rings with both ends far out, holds the
hatch's measure to the length of what the clip leaves, and cites this ADR with `Verifies:`.
`crates/arch-test` keeps ADR-0008's edges.

**Not mechanically decidable:** that `render-agg`, or another renderer, clips through no code of
its own; a reviewer holds it.
