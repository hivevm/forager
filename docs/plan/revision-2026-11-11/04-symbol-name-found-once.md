# Ticket: Find a symbol by its name in one place

## What to build

`symbol_index` in `crates/engine/src/classes.rs:461` and `resolve` in
`crates/symbolize/src/symbol.rs:88-97` both look up a symbol by its name as `msGetSymbolIndex`
does: the first symbol whose name matches without regard to case, numbered from 1. PR #172 named
the second resolver among its doubts. Let `symbolize` export the one lookup and the engine call
it, each keeping what it does with a name no symbol carries.

## Acceptance criteria

- [ ] Given a `SYMBOL` bound to a name in another case than the symbol's, when the engine binds it
  and `symbolize` resolves it, then both give the same index.
- [ ] Given the workspace, when the comparison of a symbol's name is searched for, then it is
  written once, in `symbolize`.

## Read first

- Touches: `crates/engine/src/classes.rs`, `crates/symbolize/src/symbol.rs`
- [ADR-0008](../../adr/0008-ports-and-adapters-crates.md) (the engine may depend on `symbolize`),
  `docs/semantics/style-symbol.md`.
- Overlap: `docs/plan/revision-2026-11-11/06-scale-passed-as-one.md` edits `classes.rs` too.

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests above decide it; two crates, one revert; nothing ADR-0006 keeps with the
human. A wrong result shows as a failing test of either crate. Stop after two failed attempts.
