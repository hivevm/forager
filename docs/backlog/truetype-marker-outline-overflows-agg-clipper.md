# A TrueType marker's outline panics in agg-rust's clipper on a subtraction that overflows

**Observed:** ticket revision-2026-11-16/01 (2026-11-16). The render fuzz target, given a TrueType
symbol at its points, panicked about 7,400 runs into its first 300-second run from
`fuzz/seeds/render/`:

```text
thread '<unnamed>' panicked at agg-rust-1.2.0/src/rasterizer_sl_clip.rs:139:75:
attempt to subtract with overflow
```

The stack runs from `Layer::draw_points` (`crates/engine/src/lib.rs:591`) through
`AggRenderer::glyphs` (`crates/render-agg/src/glyphs.rs:65`) into `fill_glyphs` (`:77`), with a
source of `ConvContour<ConvCurve<…>>`: the glyph's outline, the `OUTLINECOLOR` at its width, not
its fill. In the clipper, `y2 - y1` of two `i32` subpixel coordinates overflows in case 2,
`x1 > clip.x2`. cargo-fuzz builds with debug assertions, so overflow checks are on; whether the
product's own build panics there too or wraps is not checked.

A likely cause, not confirmed: `within_glyph_reach` checks the glyph's path, but `ConvContour`
then widens it by the outline width. That width is `trunc(OUTLINEWIDTH) + 1`, so any finite
`OUTLINEWIDTH`, which the fuzz input gives arbitrarily, can push the contour's vertices past the
range the rasterizer's `i32` coordinates hold. Whether a mapfile can reach it the same way, by a
large `OUTLINEWIDTH` on a TrueType symbol, is not checked.

**Input** (138 bytes, `cargo fuzz run render <file>` reproduces it on the target of ticket
revision-2026-11-16/01):

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

**Where:** `crates/render-agg/src/glyphs.rs` (`glyphs`, `within_glyph_reach`, `fill_glyphs`);
agg-rust 1.2.0's `rasterizer_sl_clip.rs`.

**What a change weighs:**

- **Bound the outline before it is widened:** test the reach of the path grown by the outline
  width, or leave out an outline wider than the image's reach, as the clip of ADR-0027 bounds
  other paths. MapServer's AGG would draw such an outline; what it draws there would decide which.
- **Saturate the coordinates the rasterizer is handed,** as the brushed line's wrapper keeps its
  start within `i32` (ebc0a4c5).
- The input becomes a seed of `fuzz/seeds/render/` in the change that fixes it.
