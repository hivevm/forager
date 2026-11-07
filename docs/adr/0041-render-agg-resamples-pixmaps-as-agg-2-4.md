# ADR-0041: `render-agg` resamples a scaled or turned pixmap with a port of AGG 2.4's bilinear clip filter and linear interpolator

- **Status:** 🟢 accepted
- **Date:** 2026-11-07
- **Deciders:** Markus Brigl
- **Applies to:** the resampling of a pixmap that `crates/render-agg` draws scaled or turned: its span generator, its interpolator, and the rounding of both

## Context

A pixmap symbol drawn at a style's `SIZE` or `ANGLE` is resampled. MapServer at the pin
(`rel-8-6-6`, `7515cccf`) does it in `agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 646–702): where
the rotation is neither 0 nor 2π or the scale is not 1, it fills a square about the point through
`span_image_filter_rgba_bilinear_clip` over `span_interpolator_linear<>`, from the AGG 2.4 it
bundles under `src/renderers/agg/`; otherwise it blends the image unscaled with `blend_from`, as
pixmap-symbols/03 built. The same function draws every pixmap of a point: a marker, a marker
along a line, a label's `labelpnt` marker.

[ADR-0015](0015-agg-rust-rasterizer.md) rasterizes with agg-rust, a port of AGG 2.6, and agg-rust
1.2.0 has both span generators. Read against the pin, they compute something else:

- **The filter.** AGG 2.4's `span_image_filter_rgba_bilinear_clip::generate`
  (`agg_span_image_filter_rgba.h`) starts each channel's sum at
  `image_subpixel_scale * image_subpixel_scale / 2`, 32768, before it adds the four weighted
  pixels and shifts right by 16, in the branch where all four pixels lie inside the image and in
  the branch at its edge: it rounds. agg-rust's `SpanImageFilterRgbaBilinearClip::generate`
  (`src/span_image_filter_rgba.rs`) starts both sums at 0: it truncates, and a resampled channel
  comes out up to 1 lower. A pixel weighted half and half between 0 and 1 is 1 in AGG 2.4 and 0
  in agg-rust.
- **The interpolator.** `span_interpolator_linear::begin` rounds both ends of a span with
  `iround(t * 256)`. MapServer's `agg_basics.h` defines `iround` as `lrint(v)` under
  `HAVE_LRINT`, which `CMakeLists.txt` (`check_function_exists("lrint" HAVE_LRINT)`) and
  `mapserver-config.h.in` (`#cmakedefine HAVE_LRINT 1`) set on every platform with `lrint`, and
  `mapagg.cpp` includes `mapserver.h`, which includes `mapserver-config.h`, before the AGG headers.
  `lrint` rounds half to even; agg-rust's `basics::iround` is `(v ± 0.5) as i32`, half away from
  zero. They part where `t * 256` falls on a half, which a scale such as 0.5 reaches.

[ADR-0034](0034-render-agg-blends-as-agg-2-4.md) met the same kind of difference in the blender
and ported AGG 2.4's arithmetic into `render-agg`, while agg-rust kept rasterizing. Fidelity (Q-4)
and the geometry and symbols cases of G-6 are held against MapServer's own pixels, and a channel
off by one on every resampled pixel is a difference the metric counts at its edges and the label
and symbol cases accumulate. The tickets after pixmap-symbols/04, GIF pixmaps, pixmaps along a
line, and a pixmap under a label, draw through the same function.

## Decision

We will resample every pixmap that `render-agg` draws scaled or turned at a point with a port of
AGG 2.4's `span_image_filter_rgba_bilinear_clip` and `span_interpolator_linear` as MapServer
bundles them, its `iround` as `lrint` from `style-ir`'s `c` module narrowed to an `int`, while
agg-rust keeps rasterizing the square they fill.

**Out of scope:** the rasterizer, the scanlines, and the coverage of the square, which stay
agg-rust's under ADR-0015; the blend of the resampled span, which ADR-0034 decides; a line
patterned with an image (`agg2RenderLineTiled`), which resamples through
`pattern_filter_bilinear_rgba8` and is a decision of its own; a polygon's pixmap fill, whose tile
MapServer builds at its size and repeats without resampling; the affine transform's arithmetic
(`trans_affine`), which `symbolize` computes with the port of `multiply` it already had and an
`invert` of its own, its sine and cosine from `libm` as every angle of the core takes them (Q-2),
and which the stated coefficients of pixmap-symbols/04 hold to AGG 2.4's.

## Alternatives considered

- **agg-rust's filter and interpolator as they are** — no code of our own, but every resampled
  channel may come out one lower than MapServer's and some spans start a subpixel off; written
  down as a deviation, it would stand against Q-4 for every pixmap path to come.
- **A patch to agg-rust upstream** — the right home for AGG 2.6's arithmetic, but agg-rust ports
  2.6 on purpose, and 2.6 rounds as it does; MapServer's 2.4 with `lrint` is ours to match, and a
  fork pinned in `Cargo.lock` brings a source outside crates.io against
  [ADR-0012](0012-crate-supply-chain.md).
- **A vendored copy of agg-rust with the two changes** — the same as a fork, in the tree.
- **Resampling with the `image` crate or a filter of our own** — a new dependency or a new
  algorithm, and neither is MapServer's.
- **Narrowing the decision to pixmap-symbols/04** — the next three pixmap tickets would take the
  same decision again; the maintainer chose one decision for every pixmap drawn at a point.

## Sources / Prior art

- MapServer `rel-8-6-6` (`7515cccfe75fb7554ea45bc87ba5eb4b6126db5d`), read on 2026-11-07:
  `src/mapagg.cpp` (`agg2RenderPixmapSymbol`, 646–702; `agg2RenderLineTiled`),
  `src/renderers/agg/include/agg_span_image_filter_rgba.h`,
  `src/renderers/agg/include/agg_span_interpolator_linear.h`,
  `src/renderers/agg/include/agg_basics.h` (`iround`), `src/mapserver.h`,
  `src/mapserver-config.h.in`, `CMakeLists.txt` line 144.
- agg-rust 1.2.0 from crates.io: `src/span_image_filter_rgba.rs`
  (`SpanImageFilterRgbaBilinearClip`), `src/span_interpolator_linear.rs`, `src/basics.rs`
  (`iround`).
- The handoff record of pixmap-symbols/04
  (`docs/plan/pixmap-symbols/04-pixmap-scaled-and-rotated.md`), which found both differences;
  [ADR-0034](0034-render-agg-blends-as-agg-2-4.md), the precedent.

## Consequences

- Positive: a scaled or turned pixmap matches MapServer's to the bit wherever the transform and the
  blend do, and the pixmap tickets after pixmap-symbols/04 inherit it without a decision of their
  own.
- Positive: the port reuses `style-ir`'s `lrint` ([ADR-0037](0037-c-number-reading-in-style-ir.md)),
  so C's rounding is still written once.
- Negative / trade-offs: two span generators of our own in `render-agg`, a few hundred lines that
  agg-rust's updates no longer maintain; a fix agg-rust makes to its filter does not reach us.
- Negative / trade-offs: agg-rust's own filter stays in the dependency for nothing; a reader may
  take the port for a duplicate, which this record answers.
- Follow-ups: the resampling of a line patterned with an image, when a ticket draws one; whether
  `libm`'s sine and cosine match glibc's at every angle, of which the tests hold the angles they
  state.

## Enforcement

Unit tests of `crates/render-agg` cite this ADR with `Verifies: ADR-0041`: a resampled pixel whose
expected bytes are derived by hand from AGG 2.4's `generate`, a pixel weighted half and half
between channel values 0 and 1 giving 1 (agg-rust gives 0), in the branch inside the image and
the branch at its edge; and an interpolator span at a scale of 0.5 whose subpixel ends are those
of `lrint`, a half rounded to even, stated as literals. The engine's reference images of
pixmap-symbols/04, a pixmap at `SIZE 55 ANGLE 45` and one at `SIZE 16`, hold the whole path, and
`crates/arch-test/tests/c_numbers.rs` keeps the rounding in `style-ir`'s `c` module.
