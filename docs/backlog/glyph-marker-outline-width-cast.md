# A TrueType marker's outline width is cut to an integer without C's cast past `int`

**Observed:** the review of revision-2026-11-16 ticket 16 (2026-11-16). `glyph_op` stands in for
drawGlyphMarker's `int olwidth` with `s.outline_width.trunc() + 1.0`, which keeps a width past
`int` as it is. MapServer's cast of such a width is undefined in C and gives `INT_MIN` on x86, so
its `olwidth + 1` is hugely negative and its contour shrinks the glyph, where Forager's, bounded by
the renderer as a stroke is, grows it to cover the image. Only a `WIDTH` and `MAXWIDTH` past
`int` reach it; below that the two agree.

**Where:** `crates/symbolize/src/glyph_marker.rs` (`glyph_op`); the C cast helpers of
`crates/style-ir/src/c.rs` (IR-c-ports-in-style-ir), which a label's outline width already goes
through (`ms_nint` in `crates/engine/src/labels.rs`).

**What a change weighs:**

- **`int_cast` of `c.rs` in `glyph_op`,** the helper that already ports x86's cast with
  `INT_MIN` past an `int`: the marker matches MapServer's build on the
  noise run's runner at every width, at the cost of drawing a shrunk contour for a width meant
  huge, and of following behaviour C leaves undefined.
- **Keeping the truncation:** no work; the drawing differs from MapServer's only past `int`,
  where no reference case reaches.
