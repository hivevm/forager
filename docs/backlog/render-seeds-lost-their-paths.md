# Three seeds of the render target no longer reach the path they were made for

**Observed:** while checking the render target's seeds for ticket 01 of revision-2026-11-16
(2026-11-17), with temporary probes at the paths, against the target of `main` at d6691adf.

- `far-end-rounds-to-its-parameter` and `far-end-rounds-to-its-parameter-slow`, the inputs of the
  timeout 5e8fcf82 fixed, reach no segment in `forager_render::clip::segment` with a coordinate past
  1e15 pixels. They pass the layer transform and the label cache, but reach no draw op: the far end
  that made them slow is gone.
- `pixmap-tiled-fill`, added by c39b8450, never calls `AggRenderer::tiled_fill`.

When they lost it was not bisected. Likely causes are the fields `Input` gained after them and the
`Cache` and layer transform tree the target later read from the same bytes. Each changed what an
older seed decodes, and the seed convention does not ask for a seed to be checked again when its
target's decoding changes. The paths
are not unfuzzed: the seeds `truetype-fill` and `buffer-edges-too-short` reach `tiled_fill`, and
`truetype-fill` reaches a segment with a coordinate past 1e15. But the regression seed of 5e8fcf82
no longer guards the input it was named for.

**Where:** `fuzz/seeds/render/`; `docs/CONVENTIONS.md`, the fuzz seed convention.

**What a change weighs:** re-deriving the three seeds is a short fuzz run each, against a temporary
panic at the path. Keeping them from going stale again would need a check that each seed still
reaches its path, such as a test of the fuzz workspace that runs each seed and asserts a counter at
the path. That is more than the convention asks today.
