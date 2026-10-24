# ADR-0015: The reference renderer rasterizes with agg-rust in `render-agg`, behind clipping that keeps every coordinate near the image, with `unsafe` allowed in that crate alone

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `crates/render`, `crates/render-agg`, and the `unsafe_code` lint of the workspace

## Context

Forager's output is judged against MapServer's reference images within a tolerance (G-6, Q-4).
MapServer rasterizes with Anti-Grain Geometry (AGG), so a renderer built on AGG has the best
chance of matching its anti-aliasing, line joins, and coverage. The renderer runs in the browser
(Q-1), deterministically (Q-2), and on untrusted data (Q-3, Q-9).

agg-rust 1.2.0 is a pure-Rust port of AGG 2.6: BSD-3-Clause, no dependencies, and it builds for
`wasm32-unknown-unknown`. A probe on 2026-10-23 established three things beyond its documentation:

- A pixel buffer is attached only through `unsafe fn RowAccessor::new_with_buf`, taking a raw
  pointer; the crate has no safe constructor, and itself uses about ninety `unsafe` blocks.
- Coordinates around 1e12 to 1e15 make the rasterizer run for minutes; with its clip box set, they
  overflow an integer instead, which panics in a debug build. NaN and coordinates moderately
  outside the image draw nothing and do no harm.
- The crate is young (first release February 2026) and has one main maintainer.

MapServer meets the second point the same way: it clips shapes to the image and a buffer before
they reach AGG.

## Decision

We will rasterize the display list with agg-rust in `render-agg`, behind the renderer trait of
`render`, under three safeguards:

- **Clipping.** Before a path reaches agg-rust, it is clipped to the image enlarged by a margin
  of the widest stroke, so that no coordinate handed to agg-rust lies further than that margin
  outside the image, and non-finite coordinates are dropped.
- **`unsafe` in one crate.** The workspace denies `unsafe_code`; `render-agg` alone allows it,
  on the one function that attaches agg-rust to its pixel buffer, with a `SAFETY:` comment.
- **Fuzzing.** The renderer is a target of the weekly fuzz run of Q-3.

**Out of scope:** text rendering and fonts; raster images; a second backend.

## Alternatives considered

- **tiny-skia** — mature, a safe API, wide use; not AGG, so the distance to MapServer's reference
  images is likely larger, against Q-4.
- **agg-rust after a fidelity spike** — would compare a few msautotest images first; the
  rendering criteria of G-6 measure it anyway, and the trait keeps the backend replaceable.
- **Our own scanline rasterizer** — the same fidelity work AGG already carries, done again.

## Sources / Prior art

- agg-rust, <https://github.com/larsbrubaker/agg-rust> (1.2.0, 2026-10-11, BSD-3-Clause).
- AGG 2.6, <https://github.com/ghaerr/agg-2.6>.
- MapServer's AGG renderer and its shape clipping before drawing, `src/mapagg.cpp` and
  `src/mapprimitive.c` (`msClipPolygonRect`, `msClipPolylineRect`), release 8.6.6.

## Consequences

- Positive: the renderer most likely to meet Q-4; no dependencies; one backend for browser and
  server.
- Negative / trade-offs: memory safety of the renderer rests on agg-rust's `unsafe` code and on
  our clipping; a young crate with one maintainer may stall, in which case the trait lets another
  backend replace it.
- Follow-ups: whether to fork or vendor agg-rust if upstream stalls.

## Enforcement

Tests in `render-agg` cite this ADR with `Verifies:`: a path with coordinates of 1e15 and with
non-finite coordinates renders within a second without a panic, and the clip keeps a path inside
the margin. The dependency test of ADR-0009 fails on `#[allow(unsafe_code)]` in any crate but
`render-agg`.
