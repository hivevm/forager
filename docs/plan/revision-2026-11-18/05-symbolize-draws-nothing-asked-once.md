# Ticket: A symbol style that draws nothing is told by one predicate

## What to build

Ticket 07 of revision-2026-11-16 put one `Mark` per kind (`crates/symbolize/src/mark.rs`). It
left one test written four times. `(s.fill.is_none() && s.outline.is_none()) || s.scale == 0.0`,
"this symbol style draws nothing", stands in:

- `symbol.rs` (`render`);
- `glyph_marker.rs`;
- `mark.rs` (`Styled::of`'s glyph arm).

`mark.rs`'s `coloured` tests the colour half alone.

A predicate of `SymbolStyle` in `symbol.rs` replaces them, or two where the colour is meant apart
from the scale, as in `coloured`. No drawing changes. This is a `refactor`.

## Acceptance criteria

- [ ] Given `crates/symbolize/src`, when it is searched for `fill.is_none() && s.outline.is_none()`,
  then only the predicate holds it, and `coloured` calls the predicate.
- [ ] Given the tests of `forager-symbolize` and the engine, and the msautotest harness, when they
  run, then all pass, no test changed.

## Read first

- Touches: `crates/symbolize/src/{mark,symbol,glyph_marker}.rs`
- ADR-0044; `docs/CONVENTIONS.md` (a commit typed `refactor`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the suite decides it, and a wrong result costs one revert. It touches no count of
Q-9 and no tile. Stop when a test's expected value would change; hand off with the difference
named.
