# Ticket: A glyph's outline, a marker's or a label's, is widened no further than a stroke

## What to build

The render fuzz target of ticket 01 panicked about 7,400 runs into its first 300-second run:

```text
thread '<unnamed>' panicked at agg-rust-1.2.0/src/rasterizer_sl_clip.rs:139:75:
attempt to subtract with overflow
```

The stack runs from `Layer::draw_points` through `AggRenderer::glyphs`
(`crates/render-agg/src/glyphs.rs`) into `fill_glyphs`. The source is a `ConvContour`, the
glyph's outline in its `OUTLINECOLOR`. `glyphs` hands the outline's width to
`ConvContour::set_width` as it is (`glyphs.rs`, `contour.set_width(outline.width)`). It checks
only that the width is finite. `within_glyph_reach` (`crates/render/src/clip.rs`) bounds the
glyph's path before the contour widens it. A stroke's width goes through `clip::drawn_width`
first (`crates/render-agg/src/lib.rs`, `stroke`), which keeps the margin within agg-rust's
fixed-point coordinates. The outline's width does not.

The mapfile frontend refuses only a negative `OUTLINEWIDTH`, a style's
(`crates/mapfile/src/translate/style.rs`) and a label's (`crates/mapfile/src/translate/label.rs`).
So a mapfile with a TrueType symbol and a large `OUTLINEWIDTH` likely reaches the same panic. A
label with a large `OUTLINEWIDTH` reaches the same `set_width` too, since the engine hands the
label's outline to `glyphs` as a `GlyphOutline` (`crates/engine/src/labels.rs`). Each is a Q-3
failure in the browser.

The outline's width goes through the same bound as a stroke's, `drawn_width` on the image's size,
before `set_width`. A test of `render-agg` draws a glyph with a huge outline width and expects no
panic and an image covered by the outline's colour, as a stroke that wide covers it. The crash
input becomes a seed of `fuzz/seeds/render/` once ticket 01's target can read it. Ticket 01 adds
it, and drops the backlog entry it filed for this crash.

The 138-byte input, as ticket 01's worker filed it, for `cargo fuzz run render <file>` on the
target of `ticket/revision-2026-11-16/01-render-fuzz-fidelity-paths` (head 8c2bfb31):

```text
60 0b 01 37 ff ff ff ff ff 00 fe 00 00 40 00 ff
06 00 21 74 69 6f 6e 22 2c 20 22 66 65 61 74 75
72 65 73 79 6d 62 6f 05 6c 73 20 7b 22 74 79 70
65 22 3a 20 20 20 20 20 20 20 20 20 20 20 a0 5b
5b 31 30 2c 2c 20 5b 32 30 2c 20 65 72 74 69 65
73 22 3a 20 7b 22 6e 61 6d 25 37 5d 2c 20 5b 39
34 65 22 70 72 65 71 9a 61 74 75 9e 8b 75 72 65
43 6f 6c 6c 65 63 74 69 6f 6e 22 30 5d 5d 5d 7d
7d 0a 5d fa 85 7f 7f 7f 47 00
```

## Acceptance criteria

- [ ] Given a glyph path that lies on the image and an outline width of `1e12`, when
  `AggRenderer` draws it with debug assertions on, then it does not panic, and every pixel of
  the image is the outline's colour.
- [ ] Given a glyph path within `within_glyph_reach` but off the image, and an outline width of
  `1e12`, when `AggRenderer` draws it, then it does not panic.
- [ ] Given an outline width of 1 or 2, when the glyph is drawn, then the image is the same as
  before the change: the existing TrueType tests and the msautotest cases with TrueType symbols
  pass unchanged.
- [ ] Given a mapfile with a TrueType symbol at a point and `OUTLINEWIDTH 1e12`, when the engine
  renders it, then it returns an image and no panic.
- [ ] Given a mapfile with a label of `OUTLINEWIDTH 1e9`, when the engine renders it, then it
  returns an image and no panic. The width fits an `int` after `MS_NINT`
  (`crates/engine/src/labels.rs`), so it reaches the bound; `1e12` wraps negative there, as in
  MapServer, and would pass without the fix.
- [ ] Given ticket 01's branch rebased onto this change, when the crash input above is run by
  `cargo fuzz run render`, then it does not panic.

## Read first

- Touches: `crates/render-agg/src/glyphs.rs`, its tests, `crates/engine/tests/truetype_marker.rs`,
  `crates/engine/tests/labels.rs`
- ADR-0025 (the clip and its margin), ADR-0044 (the glyph's outline), ADR-0026 (agg-rust);
  `docs/SPECIFICATION.md` Q-3; `crates/render/src/clip.rs` (`drawn_width`, `within_glyph_reach`).

## Blocked by

- None (can start immediately)

## Mode

unattended: a test decides it, it changes one call in one crate, and a wrong result costs one
revert. Stop if a TrueType reference image moves. That would mean MapServer's outline is drawn
differently at a width this bound reaches, and the maintainer decides it.
