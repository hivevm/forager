# Ticket: Port MapServer's `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, `msStringSplit`, and C's string end once

## What to build

[ADR-0017](../../adr/0017-c-numbers-in-style-ir.md) ports C's reading of numbers once, in
`forager_style_ir::c`, so that a port that differs from the C does so in one place. Other small
ports of `mapserver.h`, of MapServer, and of C are written more than once:

- `MS_MIN` and `MS_MAX`, which keep their second operand where a comparison fails (NaN): `ms_min`
  and `ms_max` in `crates/symbolize/src/bounds.rs` (with the test that states the NaN case,
  `ms_min_and_ms_max_keep_their_second_operand_where_they_cannot_compare` in
  `crates/symbolize/src/bounds/tests.rs`), in `crates/label/src/cache.rs`, and a third copy in the
  test `crates/label/tests/cache_pairwise.rs`; written inline in `crates/label/src/size.rs`
  (`glyph_size`), in `crates/label/src/place.rs` (`fast_bounds` and `member_box`), in
  `crates/symbolize/src/geom_transform.rs` (the padding of `GEOMTRANSFORM bbox`), and, as
  `MS_MAX`, in `crates/engine/src/labels.rs` (the offset from a label's marker).
- `MS_DEG_TO_RAD`: `DEG_TO_RAD` in `crates/symbolize/src/style.rs`, which the engine imports to
  turn a label, and `PI / 180.0` in `crates/label/src/place.rs`. In `f64` the two are the same
  value, `0.017453292519943295`, so no value moves; only the second definition goes.
- A C string's end at its first NUL: `c_bytes` and `c_str` in `crates/engine/src/classes.rs`,
  beside the selection of classes, `c_string` in `crates/expr/src/lib.rs`, and inline in
  `Scanner::new` in `crates/mapfile/src/expression.rs` and in the reading of a field's value in
  `crates/adapter-shapefile/src/dbf.rs`.
- `msRectOverlap`: `overlaps` in `crates/engine/src/inline.rs`, `Rect::overlaps` in
  `crates/adapter-shapefile/src/record.rs`, `rect_overlap` in `crates/label/src/cache.rs`, and a
  copy in the test `crates/label/tests/cache_pairwise.rs`.
- `msStringSplit`: `split_spaces` in `crates/engine/src/classes.rs` and `split_items` in
  `crates/mapfile/src/translate/feature.rs`, at different separators.

Move each into one place that every crate needing it may depend on, and let the sites call it.
`style-ir`'s `c` module, or a module beside it, is the draft's place: `symbolize`, `engine`,
`expr`, `mapfile`, and `adapter-shapefile` depend on `style-ir` already, `label` does not.
[ADR-0008](../../adr/0008-ports-and-adapters-crates.md) permits `label` to depend on `style-ir`,
so the edge is new to `crates/label/Cargo.toml` but within the table of
`crates/arch-test/tests/dependencies.rs`.

The `GEOMTRANSFORM bbox` padding converts its `MS_MAX` with C's `(int)` cast, and `glyph_size` and
`placed_with_buffer` are sites of `docs/plan/revision-2026-10-31c/03-c-int-cast-once.md` as well:
that ticket ports the cast, this one the macros, and whichever lands second takes the other's
port at those sites.

## Acceptance criteria

- [ ] Given `MS_MIN` and `MS_MAX` with a NaN operand, when they are called, then each returns the
  second operand, with a test that states it, and every site above calls the one port.
- [ ] Given the workspace, when it is searched, then `PI / 180` and a definition of `ms_min`,
  `ms_max`, `msRectOverlap`, `msStringSplit`, or a C string's end appear nowhere outside the one
  port.
- [ ] Given the workspace after the change, when `scripts/check-all.sh` runs, then it is green and
  no test or reference image changed.

## Read first

- Touches: `crates/style-ir/src/c.rs`, `crates/symbolize/src/bounds.rs`,
  `crates/symbolize/src/style.rs`, `crates/symbolize/src/geom_transform.rs`,
  `crates/label/src/cache.rs`, `crates/label/src/size.rs`, `crates/label/src/place.rs`,
  `crates/label/tests/cache_pairwise.rs`, `crates/label/Cargo.toml`,
  `crates/engine/src/labels.rs`, `crates/engine/src/classes.rs`, `crates/engine/src/inline.rs`,
  `crates/expr/src/lib.rs`, `crates/mapfile/src/expression.rs`,
  `crates/mapfile/src/translate/feature.rs`, `crates/adapter-shapefile/src/record.rs`,
  `crates/adapter-shapefile/src/dbf.rs`.
- [ADR-0017](../../adr/0017-c-numbers-in-style-ir.md),
  [ADR-0008](../../adr/0008-ports-and-adapters-crates.md);
  `crates/arch-test/tests/dependencies.rs`; `src/mapserver.h` at the pin.
- `docs/plan/revision-2026-10-31c/03-c-int-cast-once.md`, which drafts the change of ADR-0017
  this ticket's placement may need, and carries it.

## Blocked by

- None (can start immediately)

## Mode

with the human, on one question: do these ports go into `style-ir`'s `c` module, which grows what
ADR-0017 places there and gives `label` a direct dependency on `style-ir`, or into another place
the maintainer names? The ADR part of the answer is drafted in
`docs/plan/revision-2026-10-31c/03-c-int-cast-once.md`, not here.
