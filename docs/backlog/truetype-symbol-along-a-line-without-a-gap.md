# A TrueType symbol along a line without a gap is drawn as a plain line, not tiled

**Observed:** the review of fidelity ticket 06 (2026-11-15). For a style whose `GAP` is 0,
msDrawLineSymbol (`src/maprendering.c`) does not place markers. It asks `getTile` for a tile of
`MS_NINT(sizex × scale)` by `MS_NINT(sizey × scale)` pixels, at least 1 each, and hands it to
`renderLineTiled`, which brushes the line with it. A TrueType symbol's extent is 1 by 1, so the
tile is the scale's size, with the glyph drawn at its centre. Forager draws such a style as a plain
line in its `COLOR`, as it drew every TrueType line style before ticket 06, and warns of nothing.
`docs/semantics/symbol.md` says so under *A TrueType symbol along a line*.

**Where:** `crates/symbolize/src/line.rs`, the branch of `line_symbol` that sends a TrueType style
with a `GAP` to `glyph_marker::line_glyphs`.

**What a change weighs:** `tile::glyph_tile` already builds the tile, but its size rule is
msDrawShadeSymbol's, not this one. The renderer has no line brushed with a tile yet, so
`renderLineTiled`, AGG's line pattern, would be new to `render` and `render-agg`. The same path
serves vector, ellipse, and pixmap symbols with a `GAP` of 0, which Forager also draws as plain
lines, a pixmap with the warning `pixmap-not-drawn`. A TrueType symbol could warn likewise until
it is built. No msautotest case of the scope is known to need it.
