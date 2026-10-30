# ADR-0032: The hatch is cut from its polygon by i_overlay's boolean intersection, as MapServer's Clipper cuts it

- **Status:** 🟡 proposed
- **Date:** 2026-10-30
- **Deciders:** Markus Brigl
- **Applies to:** the hatch of `crates/render-agg`, the `i_overlay` entry of its `Cargo.toml`, the bounds of a hatch in `crates/render`, and every crate that intersects polygons

## Context

A polygon style with a hatch symbol fills the polygon with stroked lines (G-6, the category
geometry and symbols). MapServer at the pin, `msHatchPolygon` in `src/mapagg.cpp`, strokes the
dashed lines with butt caps and cuts the stroke from the polygon with the Clipper library through
`agg_conv_clipper` (`src/renderers/agg/include/agg_conv_clipper.h`): the polygon is the subject,
the stroke the clip, the operation `clipper_and` (`ctIntersection`), both read with
`clipper_even_odd`, the defaults of the converter; every coordinate is scaled by its default
`scaling_factor` of 2, that is by 10², and rounded half away from zero to an integer (`Round`),
and the result scaled back. `renderPolygonHatches` fills the result under the non-zero rule with
the rasterizer that carries the output format's gamma, `gamma_linear(0, 0.75)` by default. So a
pixel of the hatch is the area of an exact polygon intersection, and where two stroked lines or
two rings overlap the hatch is empty.

The first implementation cut the polygon itself: each ring moved onto each dash rectangle, with a
check for crossing rings and, where they crossed, an intersection of anti-aliased coverage instead,
drawn in bands of rows. Two reviews found it unsound: a comb of 2000 teeth under a dense hatch took
21.6 GB, since the moved points form long edges that cancel and that the rasterizer still walks;
a ring and its hole did not cancel exactly, leaving 4495 non-white pixels inside holes over 200
random cases; the crossing check sent ordinary clipped concave polygons, holes touching their
shell, and shared edges to the fallback, which differs along every dash edge; banding changed
pixels; and the budget per band multiplied the bound on time.

Q-2 asks for the same bytes natively and in the browser; Q-9 for a bound on memory and time; the
specification's *Assumptions* name i_overlay among the pure-Rust crates the work leans on, to be
verified by the ADR that picks it.

## Decision

We will cut a hatch's stroke from its polygon with i_overlay's integer intersection, as
MapServer's Clipper cuts it: the polygon the subject and the dash rectangles the clip, both under
the even-odd rule, each coordinate scaled by 100 and rounded half away from zero to an `i32`, on
i_overlay's fragment solver at absolute precision on one thread, the result scaled back and filled
under the non-zero rule through the gamma.

The renderer cuts only a hatch within the bounds the display list's `Hatch::is_bounded` sets on
its lines, dashes, the polygon's vertices, and an estimate of their crossings; the engine refuses
one beyond them with a diagnostic (Q-9). i_overlay is a dependency of `render-agg` alone, with its
default features off.

**Out of scope:** any other polygon boolean operation, such as a `GEOMTRANSFORM` `buffer`, which
may choose i_overlay by an ADR of its own; the gamma of the output format and of the layer, which
is not carried yet.

## Alternatives considered

- **The own cut, as tried** — the review findings above: unbounded memory on combs, slivers
  inside holes, a fallback that differs from MapServer along every dash, and banding that changes
  pixels. Fixing each is writing a polygon clipper.
- **geo's `BooleanOps`** — runs i_overlay underneath, through its floating-point adapter whose
  scale follows the input's bounds, so the rounding to a hundredth of a pixel that Clipper applies
  is not reproduced; and it brings the whole of geo for one operation.
- **Coverage only** — intersecting the anti-aliased coverage of the stroke and of the polygon, a
  product where MapServer takes an area: inside it matches, on the polygon's edge it does not, and
  `renderers/polygon-hatch.map` stays 0.54 % off against the tolerance of 0.1 %.
- **A port of Clipper 4** — the operation MapServer runs, written again in Rust: some thousands of
  lines to keep, where a maintained crate does the same intersection.

## Sources / Prior art

- MapServer rel-8-6-6: `msHatchPolygon`, `renderPolygonHatches`, `createHatch` in
  `src/mapagg.cpp`; `conv_clipper` (its default fill types, its `scaling_factor` of 2, `Round`) in
  `src/renderers/agg/include/agg_conv_clipper.h`.
- i_overlay 9.1.0, <https://crates.io/crates/i_overlay>, <https://github.com/iShape-Rust/iOverlay>:
  MIT OR Apache-2.0; eight releases from 7.0.3 on 2026-07-29 to 9.1.0 on 2026-10-14; some 10.9
  million downloads; geo's boolean operations run on it. With its
  default features, none, it depends on i_float, i_shape, i_tree, and i_key_sort, and through
  i_float on libm; `rayon` only behind `allow_multithreading`. No `-sys` crate. It builds for
  `wasm32-unknown-unknown` with the pinned toolchain, 1.99.1 (checked with `cargo build -p
  forager-render-agg --target wasm32-unknown-unknown`).
- Its README, *Integer Coordinate Limits*: an `i32` engine takes coordinates within ±2^30; the
  hatch's lie within the image grown by a margin, scaled by 100, far inside. Its integer engine
  computes in integers, single-threaded without the feature, so it computes the same on every
  target (Q-2). Its fill rules include `EvenOdd`; its solver snaps points within a squared radius
  of `2^start`, 1 unit at `Precision::ABSOLUTE`, a hundredth of a pixel, where Clipper snaps none.
- Measured on a release build, one core, a 4096 × 4096 image at 45°: on a star of 10 000 spikes
  under 141 lines the fragment solver took 1.9 s and the default 11.8 s; a star of 16 384 spikes
  alone took 2.4 s, its cost growing with the square of the vertices where many long edges overlap.
  The bounds in `crates/render/src/lib.rs` state what the most costly accepted hatch takes, some
  2 s at a peak of 430 MB.

## Consequences

- Positive: the cut is the intersection MapServer computes, holes stay white, overlapping rings
  and stroked lines cancel exactly as under Clipper, and `renderers/polygon-hatch.map` and
  `renderers/polygon-hatch-dashed.map` pass; the own clipper and its fallback go away.
- Negative / trade-offs: a dependency of five crates of one project; a hatch whose polygon has more
  than 16 384 vertices, or whose stroke crosses its edges too often by the estimate, is not drawn,
  where MapServer draws it however long it takes; i_overlay snaps within a hundredth of a pixel
  where Clipper does not.
- Follow-ups: a `GEOMTRANSFORM` `buffer` and other polygon operations may build on the same crate
  under an ADR of their own; the bounds of Q-9 follow the first measurement of the expected scale.

## Enforcement

A test in `crates/arch-test` cites this ADR with `Verifies:` and fails when a crate other than
`render-agg` depends on i_overlay. Tests in `render-agg` cite it: the polygon and the stroke
intersected under the even-odd rule, overlapping rings and pieces left empty, coordinates rounded
to a hundredth of a pixel half away from zero, and a hole left white under rotated dashes. The
conformance harness holds the two hatch cases of the category geometry and symbols.
