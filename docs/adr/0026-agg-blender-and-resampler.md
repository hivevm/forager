# ADR-0026: `render-agg` blends and resamples with ports of the AGG 2.4 that MapServer bundles, while agg-rust rasterizes

- **Status:** 🟢 accepted
- **Date:** 2026-11-07
- **Deciders:** Markus Brigl
- **Applies to:** the pixel format of `crates/render-agg` (how a covered pixel is blended, how a layer's image is laid over the map, how the alpha is divided out of the finished image) and the resampling of a pixmap it draws scaled or turned (its span generator, its interpolator, and the rounding of both)

## Context

[ADR-0025](0025-agg-rust-rasterizer.md) rasterizes with agg-rust, a port of AGG 2.6, and says
nothing about the arithmetic that blends a covered pixel into the image or resamples a pixmap.
MapServer at the pin (`rel-8-6-6`, `7515cccf`) bundles AGG 2.4 under `src/renderers/agg/`. Fidelity
(Q-4) and the cases of G-6 and G-13 are held against MapServer's own pixels, and Q-2 asks for the
same bytes everywhere, which integer arithmetic gives.

**Blending.** MapServer draws with `pixfmt_alpha_blend_rgba` over `blender_rgba_pre`: colours
premultiplied as `rgba8_pre` does (`>> 8`), blended into a premultiplied buffer, a layer with a
`COMPOSITE` block laid over the map by `blend_from`, and the alpha divided out by `saveAsPNG`,
truncating. agg-rust's own pixel formats round as AGG 2.6 does, straight or premultiplied. Each
rounding step differs by at most one, which an opaque image hardly shows: the twelve reference
images of the engine moved by at most 3 per channel. Over a transparent background the difference
grows: a pixel whose alpha is one lower than MapServer's is a large colour error once the alpha is
divided out. Measured over the 58 cases then in scope of the conformance harness on 2026-10-30:
with the port, `renderers/line_antialiased.map` matches MapServer's reference on every pixel; with
agg-rust's `PixfmtRgba32` or `PixfmtRgba32Pre`, 219 of 120 000 pixels differ, past the tolerance
of G-6. No other case changed its outcome between them.

**Resampling.** A pixmap symbol drawn at a style's `SIZE` or `ANGLE` is resampled.
`agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 646–702) does it where the rotation is neither 0 nor
2π or the scale is not 1: it fills a square about the point through
`span_image_filter_rgba_bilinear_clip` over `span_interpolator_linear<>`; otherwise it blends the
image unscaled with `blend_from`. The same function draws every pixmap of a point: a marker, a
marker along a line, a label's `labelpnt` marker. agg-rust 1.2.0 has both span generators, but
read against the pin they compute something else:

- **The filter.** AGG 2.4's `span_image_filter_rgba_bilinear_clip::generate`
  (`agg_span_image_filter_rgba.h`) starts each channel's sum at
  `image_subpixel_scale * image_subpixel_scale / 2`, 32768, before it adds the four weighted
  pixels and shifts right by 16, inside the image and at its edge alike: it rounds. agg-rust's
  `SpanImageFilterRgbaBilinearClip::generate` starts both sums at 0: it truncates, and a
  resampled channel comes out up to 1 lower. A pixel weighted half and half between 0 and 1 is 1
  in AGG 2.4 and 0 in agg-rust.
- **The interpolator.** `span_interpolator_linear::begin` rounds both ends of a span with
  `iround(t * 256)`. MapServer's `agg_basics.h` defines `iround` as `lrint(v)` under
  `HAVE_LRINT`, which `CMakeLists.txt` and `mapserver-config.h.in` set on every platform with
  `lrint`, and `mapagg.cpp` includes `mapserver.h`, which includes `mapserver-config.h`, before
  the AGG headers. `lrint` rounds half to even; agg-rust's `basics::iround` is `(v ± 0.5) as i32`,
  half away from zero. They part where `t * 256` falls on a half, which a scale such as 0.5
  reaches.

A channel off by one on every resampled pixel is a difference the metric counts at its edges and
the label and symbol cases accumulate.

## Decision

We will port into `render-agg` the blender and the resampler of the AGG 2.4 that MapServer bundles
at the pin, while agg-rust keeps rasterizing.

- **Blending.** `render-agg` must blend with a pixel format of its own that ports AGG 2.4's
  `pixfmt_alpha_blend_rgba` and `blender_rgba_pre`, over a premultiplied buffer. It must lay a
  layer's image over the map as `blend_from` does. It must divide the alpha out of the finished
  image as `saveAsPNG` does, truncating.
- **Resampling.** `render-agg` must resample every pixmap it draws scaled or turned at a point
  with a port of AGG 2.4's `span_image_filter_rgba_bilinear_clip` and `span_interpolator_linear`.
  The port's `iround` must be `lrint` from `style-ir`'s `c` module, narrowed to an `int`.
- **Rasterizing.** agg-rust keeps the rasterizer, the scanlines, and the coverage, of a path and
  of the square a resampled pixmap fills.

**Out of scope:**

- the rasterizer, the scanlines, and the coverage, which stay agg-rust's under
  [ADR-0025](0025-agg-rust-rasterizer.md);
- the `unsafe_code` allowance of [ADR-0025](0025-agg-rust-rasterizer.md), which the port no longer
  needs but does not withdraw;
- compositing through pixman, which MapServer's default build does not use;
- the compositing operators of a `COMPOSITE` block other than `src-over`;
- a line patterned with an image (`agg2RenderLineTiled`), which resamples through
  `pattern_filter_bilinear_rgba8` and is a decision of its own;
- a polygon's pixmap fill, whose tile MapServer builds at its size and repeats without
  resampling;
- the affine transform's arithmetic (`trans_affine`), which `symbolize` computes with its port of
  `multiply` and an `invert` of its own, its sine and cosine from `libm` as every angle of the
  core takes them (Q-2), held to AGG 2.4's by the coefficients its tests state.

## Alternatives considered

- **agg-rust's `PixfmtRgba32Pre`** — premultiplied, no code of our own; rounds as AGG 2.6 does, and
  `line_antialiased` fails on 219 pixels over its transparent background.
- **agg-rust's straight `PixfmtRgba32`, the first blender** — the same 219 pixels, and no
  premultiplied buffer for a layer's image to be laid over the map as MapServer lays it.
- **agg-rust's filter and interpolator as they are** — no code of our own, but every resampled
  channel may come out one lower than MapServer's and some spans start a subpixel off; written
  down as a deviation, it would stand against Q-4 for every pixmap path.
- **AGG 2.4's arithmetic contributed or patched into agg-rust upstream** — the port would live
  upstream, but agg-rust follows AGG 2.6 on purpose, and 2.6 rounds as it does; Forager would
  wait on a release of a young crate with one maintainer, and a fork pinned in `Cargo.lock`
  brings a source outside crates.io against [ADR-0011](0011-crate-supply-chain.md).
- **A vendored copy of agg-rust with the changes** — the same as a fork, in the tree.
- **Resampling with the `image` crate or a filter of our own** — a new dependency or a new
  algorithm, and neither is MapServer's.
- **A resampler decided for the first scaled pixmap alone** — each pixmap path after it would take
  the same decision again; the maintainer chose one decision for every pixmap drawn at a point.

## Sources / Prior art

- MapServer `rel-8-6-6` (`7515cccfe75fb7554ea45bc87ba5eb4b6126db5d`): `src/mapagg.cpp`
  (`aggColor`, `agg2CreateImage`, `aggCompositeRasterBufferNoPixman`, `agg2RenderPixmapSymbol`
  646–702, `agg2RenderLineTiled`), `src/mapimageio.c` (`saveAsPNG`),
  `src/renderers/agg/include/agg_pixfmt_rgba.h`, `agg_color_rgba.h`,
  `agg_span_image_filter_rgba.h`, `agg_span_interpolator_linear.h`, and `agg_basics.h`
  (`iround`), `src/mapserver.h`, `src/mapserver-config.h.in`, `CMakeLists.txt` line 144; read
  2026-10-30 and 2026-11-07.
- AGG 2.6, <https://github.com/ghaerr/agg-2.6>, and agg-rust 1.2.0 from crates.io: its pixel
  formats, `src/span_image_filter_rgba.rs` (`SpanImageFilterRgbaBilinearClip`),
  `src/span_interpolator_linear.rs`, `src/basics.rs` (`iround`).
- The measurement of the blender above, and the two differences of the resampler, found in the
  work of the geometry-styles and pixmap-symbols specs.

## Consequences

- Positive: a transparent image matches MapServer's to the pixel where the rasterizer agrees, and
  a layer's opacity composites as MapServer's does.
- Positive: a scaled or turned pixmap matches MapServer's to the bit wherever the transform and
  the blend do, and every pixmap drawn at a point inherits it.
- Positive: `render-agg` attaches its buffer without `unsafe`; the resampler reuses `style-ir`'s
  `lrint` ([ADR-0017](0017-c-numbers-in-style-ir.md)), so C's rounding is still written once.
- Negative / trade-offs: Forager owns some 240 lines of blending arithmetic and two span
  generators beside agg-rust, kept in step with AGG 2.4 rather than with agg-rust's releases; a
  fix agg-rust makes to its filter does not reach us. The blender moved every reference image of
  the engine by up to 3 per channel once, when it landed.
- Negative / trade-offs: agg-rust's own filter stays in the dependency for nothing; a reader may
  take the port for a duplicate, which this record answers.
- Follow-ups: an RGB output format, for which `saveAsPNG` writes the premultiplied bytes
  unchanged; the resampling of a line patterned with an image, when a ticket draws one; whether
  `libm`'s sine and cosine match glibc's at every angle, of which the tests hold the angles they
  state.

## Enforcement

Unit tests of `crates/render-agg` cite this ADR with `Verifies:`:

- in `src/lib.rs`, a blend whose expected bytes are derived by hand from `blender_rgba_pre` (a
  polygon and a fill over a channel of 80 giving 137 and 166), the division of the alpha out of a
  premultiplied pixel as `saveAsPNG` truncates it ((3, 3, 3, 6) giving 127), and the blend of a
  pixmap and a tile;
- in `src/resample.rs`, a resampled pixel whose expected bytes are derived by hand from AGG 2.4's
  `generate`, a pixel weighted half and half between channel values 0 and 1 giving 1, inside the
  image and at its edge, and an interpolator span at a scale of 0.5 whose subpixel ends are those
  of `lrint`, a half rounded to even, stated as literals.

The conformance case `renderers/line_antialiased.map` passing holds the blender against
MapServer's reference. The engine's reference images in `crates/engine/tests/pixmaps.rs`, a
pixmap at `SIZE 55 ANGLE 45` and one at `SIZE 16`, hold the resampling path.
`crates/arch-test/tests/c_numbers.rs` keeps the rounding in `style-ir`'s `c` module.
