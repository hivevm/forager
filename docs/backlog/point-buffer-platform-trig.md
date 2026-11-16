# geo's buffer of a point takes `cos` and `sin` from the platform's libm

**Observed:** fidelity ticket 12 (2026-11-15). ADR-0041 has `buffer` computed by geo's
`buffer_with_style`, and its *Context* says the round joins reach `libm` through i_float, while
the miter join calls std's `cos` and `tan`. geo 0.33.1 buffers a point with a round cap by its own
loop (`src/algorithm/buffer.rs`, `impl Buffer for Point`), which calls `num_traits::Float::cos` and
`sin`. With num-traits' feature `std` on, as the workspace resolves it, those are std's `f64::cos`
and `f64::sin`, the platform's libm, which the workspace's `clippy.toml` bans in Forager's own code
for Q-2 and G-9. So a point's buffer, a circle of 120 segments, need not have the same last bits
natively and on wasm32. No case in scope buffers a point: a class style's tree is drawn at a point
layer's points without being evaluated, and no layer's `GEOMTRANSFORM` of msautotest buffers a
point. Its 120 vertices lie within 10⁻¹² of GEOS's, measured at the unit circle.

**Where:** `buffer` in `crates/symbolize/src/geometry.rs`; the deviation stated in
`docs/semantics/style-symbol.md`.

**What a change weighs:**

- **The point's circle computed by Forager with the `libm` crate**, as GEOS's `createCircle`
  does, from `(x + d, y)` clockwise: a few lines, the same bits on every target. ADR-0041 says
  `buffer` must be geo's, so this needs a successor of it, or a maintainer's reading that a point's
  circle is outside it.
- **Keeping geo's**: no work; a G-9 difference stays possible for a layer that buffers points.
