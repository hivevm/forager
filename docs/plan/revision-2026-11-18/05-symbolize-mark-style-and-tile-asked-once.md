# Ticket: A mark asks "draws nothing" in one place, and a fill tile carries whether it is seamless

## What to build

Ticket 07 of revision-2026-11-16 put one `Mark` per kind (`crates/symbolize/src/mark.rs`). Two
things it left are written more than once or wider than they mean:

- **"This symbol style draws nothing."** `(s.fill.is_none() && s.outline.is_none()) ||
  s.scale == 0.0` stands in `symbol.rs` (`render`), `glyph_marker.rs`, and `mark.rs`
  (`Styled::of`'s glyph arm). `mark.rs`'s `coloured` tests the colour half alone. A predicate of
  `SymbolStyle` in `symbol.rs`, or two where the scale is meant apart, replaces them.
- **`(seamless, asked): (bool, TileFor)`.** `mark::tile` and `tile::vector_tile` take the pair.
  The flag means something only for a fill tile: `line.rs` passes `false` always, and only
  `shade.rs` varies it. `TileFor::Polygon` carries it, `TileFor::Polygon { seamless }`, and the
  tuple goes.

No drawing changes. This is a `refactor`.

## Acceptance criteria

- [ ] Given `crates/symbolize/src`, when it is searched for `fill.is_none() && s.outline.is_none()`,
  then only the predicate holds it, and `coloured` calls the predicate.
- [ ] Given `mark.rs` and `tile.rs`, when they are read, then no function takes a `(bool,
  TileFor)`.
- [ ] Given the tests of `forager-symbolize` and the engine, and the msautotest harness, when they
  run, then all pass. A test that builds a `TileFor` changes only in how it builds it.

## Read first

- Touches: `crates/symbolize/src/{mark,symbol,glyph_marker,tile,shade,line}.rs`, their tests
- ADR-0044, ADR-0046; `docs/CONVENTIONS.md` (a commit typed `refactor`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the suite decides it, and a wrong result costs one revert. It moves no count of Q-9:
the tile's refusals stay where `mark::tile` records them. Stop when a count, a refusal, or a
test's expected value would change; hand off with the difference named.
