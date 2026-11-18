# Ticket: A fill tile carries whether it is seamless, in place of a flag beside the kind of tile

## What to build

`mark::tile` (`crates/symbolize/src/mark.rs`) and `tile::vector_tile`
(`crates/symbolize/src/tile.rs`) take `(seamless, asked): (bool, TileFor)`. The flag means
something only for a fill tile: `line.rs` always passes `false`, and only `shade.rs` varies it.
A test helper in `glyph_marker.rs` passes `false` too. The pair is an interface that widened by a
flag where the kind was due to carry it.

`TileFor::Polygon` carries it, as `TileFor::Polygon { seamless }`, and the tuple goes from both
signatures. The tile's key, its size, and what the cache counts against Q-9's bound on the bytes
of one render's images and tiles stay as they are. No drawing changes. This is a `refactor`.

`shade.rs` is touched here and by ticket 09 of revision-2026-11-16, which makes `shade` record
its own refused hatch. Neither blocks the other; the one that lands
second takes up the other's lines.

## Acceptance criteria

- [ ] Given `mark.rs` and `tile.rs`, when they are read, then no function takes a `(bool,
  TileFor)`.
- [ ] Given the tests of `forager-symbolize` and the engine, and the msautotest harness, when they
  run, then all pass. A test that builds a `TileFor` changes only in how it builds it.
- [ ] Given the tests of the tile cache's bound (Q-9) in `tile.rs` and `shade/tests.rs`, when they
  run, then each passes with its expected values unchanged.

## Read first

- Touches: `crates/symbolize/src/{mark,tile,shade,line,glyph_marker}.rs`, their tests
- ADR-0029, ADR-0044, ADR-0046; Q-9 (the bytes of a render's images and tiles);
  `docs/CONVENTIONS.md` (a commit typed `refactor`).

## Blocked by

- None (can start immediately)

## Mode

with the human: `mark::tile` records the tile cache's refusals against Q-9's bound, and ADR-0006
keeps the resource limits from unattended work, as it kept ticket 08 of revision-2026-11-16, which
made `mark::tile` the one call. Stop when a count, a refusal, or a test's expected value would
change; hand off with the difference named.
