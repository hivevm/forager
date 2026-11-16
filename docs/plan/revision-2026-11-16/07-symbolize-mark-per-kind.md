# Ticket: A TrueType symbol is a `Mark`, and each kind is placed through one dispatch

## What to build

`Mark` (`crates/symbolize/src/symbol.rs`) has no variant for a TrueType symbol, which ADR-0044
added. So three callers resolve the symbol and test `kind == Truetype` before they match a
`Mark`: `marker.rs`, `line.rs`, and `shade.rs`. `shade.rs` calls `mark` twice and `resolve`
twice. The fan-out over the kinds is written once per placement:

- at a point: `marker.rs`;
- along a line: `line_marker::draw`, `draw_pixmap`, which carries `too_many_arguments` already,
  and `glyph_marker::line_glyphs`. Each repeats the spent check, the style, `Placement::of`, and
  `taken`;
- as a tile: `tile::tile`, `vector_tile`, and `glyph_tile`.

`Mark` gains a variant for a glyph. A placement asks one function of the mark for its extent, its
price against the marker budget, and its drawing operation at a point and an angle. The three
placements share that function, and none tests the kind before it matches the `Mark`. This is a
`refactor`.

## Acceptance criteria

- [ ] Given the tree, when `grep -n "SymbolKind::Truetype" crates/symbolize/src` runs, then it
  finds only `Mark`'s resolution, not `marker.rs`, `line.rs`, or `shade.rs`.
- [ ] Given `draw_pixmap`, when clippy runs, then it needs no `too_many_arguments`.
- [ ] Given `cargo test --workspace` and the msautotest harness with G-9's comparison, when they
  run, then all pass unchanged.

## Read first

- Touches: `crates/symbolize/src/{symbol,marker,line,line_marker,shade,tile,glyph_marker}.rs`
- ADR-0044, ADR-0046, ADR-0029; `docs/ARCHITECTURE.md` (the `symbolize` section and its order of
  modules).

## Blocked by

- docs/plan/revision-2026-11-16/04-render-stroke-pen.md (both change the producers of a stroke)

## Mode

unattended: the existing tests and the harness decide it, it stays in one crate, and a wrong
result costs one revert. Stop if a reference image moves.
