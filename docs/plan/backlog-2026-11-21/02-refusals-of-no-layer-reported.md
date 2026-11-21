# Ticket: What the renderer refuses while it draws a fill tile or the label cache is reported

## What to build

The engine takes the renderer's refusals after each layer and reports them on that layer
(ARCH-third-party-failures-guarded). Two drawings belong to no layer, and what the renderer
refuses in them reaches no diagnostic, against Q-3:

- **A fill tile.** A renderer of its own draws the fill tile when the brush or the tiled fill
  first needs its image. Its refusals, such as a TrueType glyph of the fill tile past its reach or
  a path its clip cuts, are never taken. The renderer is dropped with them.
- **The label cache.** The engine takes what the renderer refused while drawing the cache and
  discards it, so that the next layer of `POSTLABELCACHE TRUE` is not blamed for it.

Report each such refusal on the layer it comes from: a fill tile is built for one style of one
layer, and the label cache knows each label's layer. Start with the fill tile. Find a mapfile whose
fill tile the renderer refuses part of, and make it the first test. The backlog entry found no
input that reaches the label cache's refusals. If none is found here either, carry them on the
layer in the same way, held by a unit test at the seam.

The maintainer chose to report rather than leave them unreported (backlog entry
`refusals-of-no-layer`, triaged 2026-11-21).

## Acceptance criteria

- [ ] Given a mapfile whose fill tile holds a mark its renderer refuses, when it renders, then the
      diagnostics name that refusal on the layer whose style needs the fill tile.
- [ ] Given that mapfile, when it renders again, or when the fill tile serves several features, then
      the refusal is reported once per fill tile, not per use.
- [ ] Given a refusal while the label cache is drawn, when the render ends, then it is reported on
      the layer of the label drawn, and no layer of `POSTLABELCACHE TRUE` is blamed for it.
- [ ] Given a mapfile without such refusals, when it renders, then its diagnostics do not change.

## Read first

- Touches: `crates/render/src/tile.rs`, `crates/render-agg/src/lib.rs`,
  `crates/engine/src/labels.rs`, `crates/engine/src/lib.rs`
- `Tile::image` and `built_tile`, where the fill tile's renderer is made and dropped.
- `Labels::finish`, the take at its end, and `render_with_files`, the take after each layer.
- `Refused` and `Renderer::take_refused` in `crates/render/src/lib.rs`.
- ARCH-third-party-failures-guarded; Q-3 of `docs/SPECIFICATION.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: how a refusal outside a layer's drawing travels to its layer, through the
renderer's channel or beside it, shapes ARCH-third-party-failures-guarded. The session must settle
that shape with the human before it builds the label cache's half. If no input reaches the fill
tile's refusals either, it must hand off with what it tried before building the seam.
