# What the renderer refuses outside a layer's drawing is not reported

**Observed:** ticket revision-2026-11-18/11 (2026-11-18), its review, and the maintainer. The
engine takes the renderer's refusals (`Renderer::take_refused`) after each layer and reports them
on that layer (ARCH-third-party-failures-guarded). Two drawings belong to no layer:

- **The label cache**, drawn after every layer but those of `POSTLABELCACHE TRUE`, and before
  those. The engine takes what the renderer refused while drawing it and discards it, so that the
  next `POSTLABELCACHE` layer is not blamed for it. Nothing reaches it with today's kinds of
  refusal: the text path leaves a glyph out of reach before the renderer meets it, and no input
  was found whose label box or marker the clip cuts.
- **A tile**, drawn by a renderer of its own when the brush or the tiled fill first needs its
  image (`Tile::image`). What that renderer refuses, a TrueType glyph of the tile past its reach,
  or a path its clip cuts, is never taken.

**Where:** `crates/engine/src/labels.rs` (`Labels::finish`, the take at its end),
`crates/engine/src/lib.rs` (`render_with_files`, the take after each layer),
`crates/render/src/tile.rs` (`Tile::image`), `crates/render-agg/src/lib.rs`
(`built_tile`); Q-3 of `docs/SPECIFICATION.md`.

**What a change weighs:**

- **Report them on the map**, as a warning of no layer, or on the layer whose label or tile it
  was: the label cache knows each label's layer, and a tile is built for one style of one layer.
  Either carries a refusal from a renderer that no layer drew with, through the channel or
  beside it.
- **Leave them unreported**: no work while no input reaches them; a refusal that comes to reach
  one is dropped without a diagnostic, against Q-3.
