# ADR-0034: `render-agg` blends with a port of AGG 2.4's premultiplied blender, as MapServer bundles it

- **Status:** 🟢 accepted
- **Date:** 2026-10-30
- **Deciders:** Markus Brigl
- **Applies to:** the pixel format of `crates/render-agg`: how a covered pixel is blended, how a layer's image is laid over the map, and how the alpha is divided out of the finished image

## Context

[ADR-0015](0015-agg-rust-rasterizer.md) rasterizes with agg-rust, a port of AGG 2.6, and says
nothing about the arithmetic that blends a covered pixel into the image. MapServer at the pin
bundles AGG 2.4 and draws with `pixfmt_alpha_blend_rgba` over `blender_rgba_pre`: colours
premultiplied as `rgba8_pre` does (`>> 8`), blended into a premultiplied buffer, a layer with a
`COMPOSITE` block laid over the map by `blend_from`, and the alpha divided out by `saveAsPNG`,
truncating. agg-rust's own pixel formats round as AGG 2.6 does, straight or premultiplied.

Each rounding step differs by at most one, which an opaque image hardly shows: the twelve
reference images of the engine move by at most 3 per channel. Over a transparent background the
difference grows: a pixel whose alpha is one lower than MapServer's is a large colour error once
the alpha is divided out. Measured over the 58 cases in scope of the conformance harness on
2026-10-30: with the port, `renderers/line_antialiased.map` matches MapServer's reference on every
pixel; with agg-rust's `PixfmtRgba32` or `PixfmtRgba32Pre`, 219 of 120 000 pixels differ, past the
tolerance of G-6. No other case changes its outcome between them. Q-4 asks for MapServer's output
within the tolerance; Q-2 for the same bytes everywhere, which integer arithmetic gives.

## Decision

We will blend in `render-agg` with a pixel format of our own that ports AGG 2.4's
`pixfmt_alpha_blend_rgba` and `blender_rgba_pre` as MapServer bundles them, over a premultiplied
buffer, compositing a layer's image as `blend_from` does and dividing the alpha out of the
finished image as `saveAsPNG` does, while agg-rust keeps rasterizing.

**Out of scope:** the rasterizer, the scanlines, and the coverage, which stay agg-rust's under
ADR-0015; the `unsafe_code` allowance of ADR-0015, which this port no longer needs but does not
withdraw; compositing through pixman, which MapServer's default build does not use; the
compositing operators of a `COMPOSITE` block other than `src-over`.

## Alternatives considered

- **agg-rust's `PixfmtRgba32Pre`** — premultiplied, no code of our own; rounds as AGG 2.6 does, and
  `line_antialiased` fails on 219 pixels over its transparent background.
- **agg-rust's straight `PixfmtRgba32`, as before** — the same 219 pixels, and no premultiplied
  buffer for a layer's image to be laid over the map as MapServer lays it.
- **Contribute AGG 2.4's arithmetic to agg-rust** — the port would live upstream, but agg-rust
  follows AGG 2.6 on purpose, and Forager would wait on a release of a young crate with one
  maintainer.

## Sources / Prior art

- MapServer `rel-8-6-6`: `src/renderers/agg/include/agg_pixfmt_rgba.h` and `agg_color_rgba.h`
  (AGG 2.4 as bundled), `src/mapagg.cpp` (`aggColor`, `agg2CreateImage`,
  `aggCompositeRasterBufferNoPixman`), `src/mapimageio.c` (`saveAsPNG`).
- AGG 2.6, <https://github.com/ghaerr/agg-2.6>, and agg-rust 1.2.0's pixel formats.
- The measurement above, from the work of ticket geometry-styles/08.

## Consequences

- Positive: a transparent image matches MapServer's to the pixel where the rasterizer agrees, and
  a layer's opacity composites as MapServer's does.
- Positive: `render-agg` attaches its buffer without `unsafe`.
- Negative / trade-offs: Forager owns some 240 lines of blending arithmetic beside agg-rust, kept
  in step with AGG 2.4 rather than with agg-rust's releases; every reference image of the engine
  moves by up to 3 per channel once, when the port lands.
- Follow-ups: whether to withdraw the `unsafe_code` allowance of ADR-0015 now that `render-agg`
  uses none; an RGB output format, for which `saveAsPNG` writes the premultiplied bytes unchanged.

## Enforcement

Unit tests of `crates/render-agg` cite this ADR with `Verifies: ADR-0034`: a blend whose expected
bytes are derived by hand from `blender_rgba_pre` (a polygon and a fill over a channel of 80
giving 137 and 166), and the division of the alpha out of a premultiplied pixel as `saveAsPNG`
truncates it ((3, 3, 3, 6) giving 127). The conformance case `renderers/line_antialiased.map`
passing holds the whole of it against MapServer's reference.
