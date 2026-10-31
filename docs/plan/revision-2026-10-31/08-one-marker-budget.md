# Ticket: Spend the marker budget one way on every marker path

## What to build

Two paths spend the engine's marker budget, each pricing a symbol by probing one of it:
`line_marker::draw` with a cost of at least 1, writing the budget's fields directly and drawing in
part; `draw::transformed` without that floor, through `take`, all or nothing per shape. Point layers
spend nothing. Give the budget one `price` of a symbol in a style and one `take`, used by the three
paths, a point layer's markers among them, under the wording ticket 07 of this revision settles.

## Acceptance criteria

- [ ] Given a point layer of one feature with 2,000,000 vertices and a vector marker, when it is
  drawn, then the markers past the bound are left out with the layer's diagnostic of Q-9.
- [ ] Given the tests of the budget on lines and on `GEOMTRANSFORM`, when they run, then they pass
  unchanged.

## Read first

- `MarkerBudget` and `draw` in `crates/engine/src/line_marker.rs`; `transformed` and `marker_ops` in
  `crates/engine/src/draw.rs`.

## Blocked by

- docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md

## Mode

with the human: it changes a resource limit of Q-9, which ADR-0007 keeps with the maintainer.
