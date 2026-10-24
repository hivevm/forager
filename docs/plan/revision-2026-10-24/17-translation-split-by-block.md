# Ticket: Split the translation by the block it reads, and read a token's value once

## What to build

`crates/mapfile/src/translate.rs` has grown to 1,332 lines, one `Translator` that reads the map,
layers, classes, labels, styles, symbols with their shape computation, the font set, and
projections, together with the value readers they share; its tests are one file of 1,352 lines
(`crates/mapfile/src/tests.rs`). Within it, the same reading is written several times:

- a single `[name]` binding token becomes `Bound::Binding` in `bound_choice`, `bound_string`,
  `bound_color`, `bound_number`, `bound_pair`, and `symbol`;
- a single string or word token is matched by hand in `item`, `string`, `symbol`, and
  `geom_transform`, and again in `crates/mapfile/src/lib.rs` (`named_file`) and
  `crates/mapfile/src/init_file.rs` (`named`).

The translation moves into modules by what they read — at least the symbols (`symbol_block`,
`points`, `shape`), the font set (`fonts`), and the labels — each with its tests beside it, while
`translate.rs` keeps the map, the layer, the class, and the shared value readers. A binding and
a token's string value are each read by one function. On the way, the stale first line of the
doc comment on `color` (`translate.rs:986`, "`r g b` with each in 0..=255"), which the next line
contradicts, goes, and `expression` gets its missing blank line and a doc comment. Behaviour
does not change.

## Acceptance criteria

- [ ] Given the tests of `crates/mapfile` and the msautotest tests of `crates/cli`, when they run,
  then they pass, the moved tests unchanged but for their place.
- [ ] Given the tree, when `TokenKind::Binding(name)` is searched for under
  `crates/mapfile/src/`, then it appears in the lexer, the expression translator, and one value
  reader of the translation.
- [ ] Given `translate.rs`, when its lines are counted, then it holds fewer than half of today's.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md).
- The conventions on MapServer's notice and on behaviour ported from MapServer in
  `docs/CONVENTIONS.md`: each new module that follows MapServer's code carries the notice.
- `crates/mapfile/src/translate.rs`, `crates/mapfile/src/tests.rs`.

## Blocked by

- docs/plan/revision-2026-10-24/01-diagnostics-bounded-while-collected.md
- docs/plan/revision-2026-10-24/16-projection-parameters-read-once.md

## Mode

with the human: the split moves the font set's and the symbol images' resolution against the base
context (Q-8), which is kept with the maintainer ([ADR-0007](../../adr/0007-harness.md)); a
wide move collides with every open ticket on the frontend, so it waits for both blockers.
