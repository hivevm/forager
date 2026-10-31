# Ticket: Resolve a style's symbol and shift a marker's anchor once

## What to build

Four functions of `crates/engine/src/symbol.rs` resolve a style's symbol by its index, each with
`checked_sub(1)` and `symbols.get`, and each treats the unresolved, the missing, and the default
symbol a little differently: `mark`, `is_hatch`, `default_size_of`, and `marker_size_of`. And the
shift of a marker by its symbol's anchor point, turned by its rotation, is written twice: `anchored`
in `symbol.rs` (msAdjustMarkerPos) and `anchor_offset` in `line_marker.rs`
(msImagePolylineMarkers), the same formula fed a different size. Resolve the symbol in one function
whose result says which of the cases it is, and shift by the anchor in one, each caller handing it
the size MapServer hands it.

## Acceptance criteria

- [ ] Given a style whose symbol is 0, past the symbol set, a name not resolved, or a valid index,
  when each of the four functions reads it, then it gives what it gives today, a test per case.
- [ ] Given a marker with an anchor point at a point and along a line, rotated, when drawn, then
  the display items are those of today.
- [ ] Given the engine's tests and the harness, when they run, then they pass unchanged and no
  reference image is re-blessed.

## Read first

- `crates/engine/src/symbol.rs`, `line_marker.rs`, and their tests; MapServer at the pin:
  `msAdjustMarkerPos` and `msImagePolylineMarkers` (`src/maprendering.c`); the entry on ported
  behaviour in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's tests decide its criteria, pinned first where a case has no test; an
interface-preserving refactoring inside the engine, undone by one revert; it touches none of the
parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a moved test or image; stop: when the four cases cannot be one
function without a change of what one of them draws, the session leaves the difference in this file
and hands off.
