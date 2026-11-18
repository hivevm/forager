# Ticket: `symbolize`'s module sentence, its arch-test table, and three module docs say what the code holds

## What to build

The refactors of revision-2026-11-16 moved code between the modules of `crates/symbolize` and
left their descriptions behind:

- `lib.rs`'s module sentence says "`line_marker` and `tile` import `bounds` and `symbol`", and the
  table of `crates/arch-test/tests/symbolize_modules.rs` allows `bounds` for both. Neither imports
  `bounds` any more, so the table allows more than the code needs.
- `symbol.rs`'s module doc names "a vector or an ellipse symbol". The file also holds `Mark`,
  `resolve`, `Pixmaps`, the marker sizes, and a pixmap's style, square, and rendering.
- `line_marker.rs`'s module doc opens "Symbols along a line: a vector, an ellipse, a pixmap, or a
  TrueType symbol". Since ticket 07 it places positions and keeps the marker budget, and
  `mark.rs` draws the kinds.

The table and the sentence drop `bounds` for `line_marker` and `tile`. The two module docs say
what their files hold. `docs/ARCHITECTURE.md`'s `symbolize` paragraph is checked against them
and corrected where it differs. No code changes.

## Acceptance criteria

- [ ] Given the table without `bounds` for `line_marker` and `tile`, when
  `symbolize_modules.rs` runs, then it passes.
- [ ] Given a temporary `use crate::bounds;` in `tile.rs`, when `symbolize_modules.rs` runs, then
  it fails on that import. The temporary line is not committed.
- [ ] Given the module docs of `symbol.rs` and `line_marker.rs`, when they are read beside the
  items of their files, then each names what the file holds.

## Read first

- Touches: `crates/symbolize/src/{lib,symbol,line_marker}.rs`,
  `crates/arch-test/tests/symbolize_modules.rs`, `docs/ARCHITECTURE.md` (the `symbolize` section)
- ADR-0008 (it decides the direction between crates only); the module order in `lib.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: `symbolize_modules.rs` decides the table, and a wrong result costs one revert. Stop
when a module the docs describe imports what the table would refuse; hand off with the import.
