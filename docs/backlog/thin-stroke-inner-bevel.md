# A line stroked 1 pixel wide or less does not bevel its inner joins, as agg2RenderLine does

**Observed:** fidelity ticket 12 (2026-11-15). For a stroke whose width is 1 or less,
agg2RenderLine (`src/mapagg.cpp:194-201` at `rel-8-6-6`) sets the stroker's join and its inner join
to bevel and leaves its cap as it was. Forager gives such a stroke a bevel join and a butt cap
(`line_ends` in `crates/symbolize/src/line.rs`), but leaves AGG's default inner join, a miter: the
display list does not carry an inner join. `line_ends` says the inner joins "lie within a pixel of
the path". At a sharp corner they do not: there MapServer covers the pixel at the vertex and its
neighbours fully, and Forager about 70 % of it.

Measured on two cases whose polygons are outlined 1 pixel wide:

- `renderers/polygon-as-line.map polygon-as-line.png`: 17 of 30,000 pixels differ past G-6's
  threshold, all at the polygons' sharp vertices. It passes, under G-6's 30.
- `renderers/polygon-as-line-offset.map polygon-as-line-offset.png`: 60 differ. 32 lie at the
  corners of its two `OFFSET` styles, which `buffer` does not touch, and 25 more at the corners of
  the polygons its `buffer([shape], -1)` style shrinks. It fails.

With the inner join set to bevel for every stroke of width 1 or less, tried by hand in
`Stroke::apply` of `crates/render-agg/src/lib.rs`: `polygon-as-line.png` differs in 0 pixels and
`polygon-as-line-offset.png` in 3, at the starts of the buffered rings, where GEOS's rings start
at another point than geo's. Both pass. But `misc/nested_symbolset.map` then fails, 1 of 100
pixels: its unfilled vector symbol is stroked 1 pixel wide by agg2RenderVectorSymbol, with a
stroker of AGG's defaults, a miter inner join. The renderer cannot tell the two strokes apart today.

**Where:** `crates/symbolize/src/line.rs` (`line_ends`), `crates/symbolize/src/shade.rs` and
`crates/symbolize/src/marker.rs`, which call it, `crates/render/src/lib.rs` (`DrawOp::Stroke`), and
`crates/render-agg/src/lib.rs` (`Stroke::apply`).

**What a change weighs:**

- **An inner join in `DrawOp::Stroke`**, set by `line_ends` and left at AGG's default by a vector
  symbol's stroke: the display list gains a field, every `DrawOp::Stroke` written in the tests
  with it. It decides both cases above as MapServer draws them, as far as measured.
- **Keeping the miter inner join**: no work; `polygon-as-line-offset.png` stays failing, and
  `line_ends`'s comment needs correcting.
- MapServer's stroker also carries its cap from one line to the next within an image, which
  Forager does not (`Stroke::apply`'s comment); the measurement above did not need it.
