# A brushed line's piece at a sharp turn is left undrawn below agg-rust's overflow

**Observed:** revision-2026-11-16 ticket 21 (2026-11-18), by the second review of its bound, with
probes against `brushed_line` in `crates/render-agg/src/brush.rs` on images of 4000 pixels.
`joins_within_i32` bounds each product of agg-rust's `DistanceInterpolator4` by adding the
pixel's offset `u` to the join's distance `|v|` as scalars, which is loose where the two point the
same way. Of 1500 random roads a class, with segments of 2 to 3000 pixels, it rejects paths that
agg-rust draws without an overflow:

- turns up to 30°: 0; up to 90°: 6; up to 120°: 29; up to 150°: 48; up to 170°: 63;
- a segment of 696 pixels turning about 160° into one of 2042, with a tile of 64 × 40, is
  rejected, though agg-rust draws 139 505 pixels of it; its worst product is `1.56e9` against the
  bound's `2.65e9`. Scaled, the probe rejects it from ×0.90, and agg-rust overflows from ×1.13.

The rejected piece is not drawn at all. Most other paths the bound rejects overflow on the base
before ticket 21, so the bound saves them from the panic.

**Where:** `joins_within_i32` in `crates/render-agg/src/brush.rs`.

**What a change weighs:**

- **A bound by the box the pixels lie in:** take the largest distance of a pixel to the join on
  each axis over the segment's range grown by the bound's growth, and bound the difference by
  `|(P − X) × v|` at the four corners of that box. The review's throwaway of it lowered the losses
  to 0, 9, 13, and 24, and drew 1317 pieces at its edge without a panic.
- **Each such change** needs the adversarial probe again, since the bound guards undefined
  behaviour of AGG's C++.
