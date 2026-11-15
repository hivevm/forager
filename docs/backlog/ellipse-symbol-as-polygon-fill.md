# An ellipse symbol over a polygon is filled in its colour, not tiled

**Observed:** the review of fidelity ticket 07 (2026-11-15). MapServer's `getTile`
(`src/maprendering.c`) tiles an ellipse symbol over a polygon as it tiles a vector symbol: the
ellipse is drawn once at the tile's centre and is never seamless, since msDrawShadeSymbol sets
seamless mode for a vector symbol alone. Forager still fills such a polygon plainly in the style's
`COLOR`. `docs/semantics/symbol.md` says so under *A vector symbol over a polygon*.

**Where:** `crates/symbolize/src/shade.rs`, the branch of `shade` that sends a vector symbol to
`vector_tiled`.

**What a change weighs:** `symbol::render` already draws an ellipse, and `tile::vector_tile`
already takes a non-seamless tile. Accepting `SymbolKind::Ellipse` there, with `seamless` false,
would likely be enough, with a test of the tile's size and mark. No msautotest case of the scope
is known to need it, so the harness would not show the gain. TrueType symbols over a polygon
belong to fidelity tickets 05 and 06.
