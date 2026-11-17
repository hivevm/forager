# Ticket: The canvas gets a symbol's tile and records its refusal in one call

## What to build

A tile and its refusal are fetched in two places: `canvas.rs` (`pixmap_tile`, `symbol_tile`)
for pixmaps and vector symbols, and `glyph_marker.rs` again for glyphs. The message "no tiles are
kept where it is drawn" is written three times. `tile.rs` builds the same `Key` from a
`SymbolStyle` twice. Since ticket 07, `mark::tile` (`mark.rs`) chooses the kind of tile once for a
brushed line and a shaded vector, ellipse, or glyph, but it still calls three fetchers:
`Canvas::pixmap_tile`, `Canvas::symbol_tile`, and `glyph_marker::glyph_tile`. A shaded pixmap
calls `Canvas::pixmap_tile` from `shade.rs` itself, with the image it read first.

One call of the canvas takes the style, the symbol, and whether the tile is a polygon's fill or a
line's brush. It chooses the kind, builds the key once, asks the cache, and records a refusal as
ADR-0029 and ADR-0046 have it warn. `mark::tile` and `shade.rs`'s pixmap fill call it, and
`glyph_marker.rs` keeps the glyph it draws in the tile. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `grep -rn "no tiles are kept where it is drawn" crates/symbolize/src`
  runs, then it finds one place.
- [ ] Given `tile.rs`, when it is read, then a `Key` is built from a `SymbolStyle` in one
  function.
- [ ] Given `cargo test --workspace` (the Q-9 tests of tiles and brushes among them) and the
  msautotest harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/symbolize/src/{canvas,tile,glyph_marker,mark,shade}.rs`
- ADR-0029, ADR-0046, ADR-0044; `docs/SPECIFICATION.md` Q-9.

## Blocked by

- docs/plan/revision-2026-11-16/07-symbolize-mark-per-kind.md (the tile follows the mark)

## Mode

with the human: it moves the tile cache's refusals under Q-9's bounds on tiles and brushes,
and ADR-0006 keeps the resource limits from unattended work.
