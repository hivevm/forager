# A simplified polygon is repaired by `unary_union`, which keeps another lobe than GEOS's buffer of 0

**Observed:** fidelity ticket 11 (2026-11-15). GEOS repairs a simplified polygon that is not valid
by a buffer of 0 (`DPTransformer.createValidArea`); Forager repairs it by geo's `unary_union`
(ADR-0050). On 2,467 small random valid polygons, of 6 to 9
vertices and 2 decimals, at 3, 6, and 10 units, 40 results were repaired: 33 matched GEOS 3.12.1
within 10⁻⁷ with its point count, 7 kept another lobe of a ring that crosses itself than GEOS keeps,
27 to 54 units away. For example, `62.55 29.06, 38.94 37.17, 37.4 31.93, 39.85 48.91, 55.51 87.12,
61.37 61.97, 63.26 62.71, 62.55 29.06` at 3: GEOS keeps the large lobe, `unary_union` a sliver of
area 0.47. The orientation of the input does not change the count. No case in scope simplifies a
polygon. The maintainer kept `unary_union`, with the difference stated.

**Where:** `simplify_areas` in `crates/symbolize/src/geometry.rs`; the deviation stated in
`docs/semantics/style-symbol.md`.

**What a change weighs:**

- **A buffer of 0 through geo's buffer, as JTS and GEOS repair**: measured with fidelity ticket
  12 (2026-11-15) on the same 40 repaired results, with
  ADR-0050's style of round joins of π/60. It matches GEOS on
  the same 33 as `unary_union`, within 10⁻⁷ with its point count. On the 7 others geo's buffer of 0
  is empty, where `unary_union` keeps a sliver and GEOS the large lobe: a
  simplified polygon would vanish. It repairs no closer than `unary_union`.
- **Keeping `unary_union`**: no work; the 7 of 40 stay different, and the semantics page says so.
- **A repair of Forager's own** that picks the lobe as GEOS's buffer does (how it picks it is not
  yet read from JTS's buffer code): more owned geometry code, under an ADR revision.
