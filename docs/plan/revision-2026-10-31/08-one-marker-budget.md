# Ticket: Spend the marker budget one way on every marker path

## What to build

Two paths of `crates/symbolize` spend a layer's marker budget (`MarkerBudget` in
`crates/symbolize/src/line_marker.rs`, held by the `Canvas` of `crates/symbolize/src/draw.rs`),
each pricing a symbol by probing one of it:

- `line_marker::draw`, a symbol with a `GAP` along a line, prices it at least 1 and writes the
  budget's fields `remaining` and `spent` directly.
- `transformed` in `draw.rs`, the symbols a `GEOMTRANSFORM` places, prices it through
  `marker_ops` without that floor and spends it through `MarkerBudget::take`, all or nothing per
  shape.

A point layer's markers, drawn by `marker_ops` from `draw_ops`, spend nothing. Give the budget
one price of a symbol in a style and one `take`, used by the three paths, a point layer's markers
among them, under the wording ticket 07 of this revision settles.

## Acceptance criteria

- [ ] Given a point layer of one feature with 2,000,000 vertices and a vector marker, when it is
  drawn, then the markers past the bound are left out with the layer's diagnostic of Q-9.
- [ ] Given the tests of the budget on lines and on `GEOMTRANSFORM` in `draw.rs`, when they run,
  then they pass unchanged.

## Read first

- `MarkerBudget` and `draw` in `crates/symbolize/src/line_marker.rs`; `draw_ops`, `transformed`,
  and `marker_ops` in `crates/symbolize/src/draw.rs`.

## Blocked by

- docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md

## Mode

with the human: it changes a resource limit of Q-9, which ADR-0007 keeps with the maintainer.

## Maintainer's decision (2026-11-08)

Implement it as the ticket says: one price of a symbol in a style (at least 1) and one `take`, used
by the three paths, a point layer's markers among them; the budget stays 1,000,000 points.
