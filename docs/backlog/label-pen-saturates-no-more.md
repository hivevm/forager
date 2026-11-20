# A label's pen no longer saturates, though its comment says it does

**Observed:** the review of revision-2026-11-19 ticket 05 (2026-11-19). `Layout::line` computes
the pen of a line as `rows.saturating_mul(self.line_height)`. Its comment says the product
saturates where C's `int` overflows. c39b8450 put the saturation in for a panic the weekly fuzz
runs of 2026-11-01 and 11-08 found: then the line height was an `f64` cast to `i64`, which
saturated at `i64::MAX`. Since ef639d7a, `Layout::new` makes the line height
`i64::from(int_cast(..))`, so its magnitude is at most 2^31. `rows` is bounded by the line
count. So the `i64` product overflows only past some 4.3e9 lines, which Q-9's bounds on a label's
text exclude. The saturation is dead code. The comment describes nothing the code does. Where
C's `int` product overflows, the code computes its exact value in `i64`. The seed
`label-lines-past-an-int` of the render target reaches that case. The comment already misled the
first doc of that seed's path.

**Where:** `crates/label/src/layout.rs`, `Layout::line` (the comment and the product).

**What a change weighs:**

- **A plain `i64` product,** with a comment that it holds MapServer's undefined `int` overflow
  as the exact value: no behaviour changes, and the code says what it does.
- **Modelling C's `int`,** with the low 32 bits that x86's `imul` leaves: the pen follows
  MapServer's build, at the cost of following behaviour C leaves undefined (IR-c-ports-in-style-ir
  decides the casts, not the overflow of a product).
