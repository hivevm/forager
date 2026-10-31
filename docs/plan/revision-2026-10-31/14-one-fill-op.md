# Ticket: Draw a fill and a polygon through one display item with a gamma

## What to build

`DrawOp::Fill` and `DrawOp::Polygon` in `crates/render/src/lib.rs` carry the same data and differ in
the gamma of 0.75 `render-agg` applies to one of them; every consumer matches both together
(`draw.rs`, `line_marker.rs`). [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md) notes that the
output format's `GAMMA` is not carried yet, which would add a third variant or a field. One fill
item with an optional gamma, the same on `Hatch`, with the engine setting MapServer's default.

## Acceptance criteria

- [ ] Given the reference images of `crates/engine/tests/render.rs`, when they render through the
  one item, then they are unchanged.
- [ ] Given the display list, when its items are read, then a fill is one variant.

## Read first

- [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md); `DrawOp` in `crates/render/src/lib.rs`; the
  gamma in `crates/render-agg/src/lib.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the reference images and the display-list tests decides its criteria; its scope is
`render`, `render-agg`, and their callers in the engine, and a wrong result costs one revert; it
touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts
against its cap of three; a wrong result shows as a moved reference image; stop: after two attempts
that leave `scripts/check-all.sh` red, or when a reference image moves, the session leaves its state
in this file and hands off.
