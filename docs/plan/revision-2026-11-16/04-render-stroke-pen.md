# Ticket: A stroke's pen is one value of `render`, with one polarity of antialiasing

## What to build

`DrawOp::Stroke` (`crates/render/src/lib.rs`) has grown to nine fields. Two flags came in with the
fidelity spec: `inner_join`, and `antialiased` from ticket 09. `render-agg`'s own `Stroke`
copies them field by field (`crates/render-agg/src/lib.rs`). Three of the four producers in
`symbolize` write `antialiased: true` (`shade.rs`, `symbol.rs`, `marker.rs`). The style IR holds
the opposite polarity, `Style.aliased`, and `line.rs` inverts it.

A `Pen` of `render` holds what a stroke is drawn with: width, cap, join, inner join, dashes, and
whether it is antialiased. It has a constructor for the antialiased default, so a producer names
only what differs. `DrawOp::Stroke` holds paths, `closed`, colour, and a `Pen`. `render-agg`
takes the `Pen` without copying it field by field. Whether the IR's `aliased` keeps its name is
the style document's business (ADR-0006, ADR-0013) and stays out of this ticket. This is a
`refactor`.

## Acceptance criteria

- [ ] Given the tree, when `DrawOp::Stroke` is read, then it holds a `Pen`, and no producer in
  `symbolize` spells out `antialiased: true`.
- [ ] Given `cargo test --workspace` and the msautotest harness with G-9's comparison, when they
  run, then all pass unchanged and no reference image is re-blessed.
- [ ] Given the commit, when `scripts/sensor-refactor-tests.sh` runs, then it lists only tests
  that build a `DrawOp::Stroke`, and those change only in how they build it.

## Read first

- Touches: `crates/render/src/lib.rs`, `crates/render-agg/src/lib.rs`,
  `crates/symbolize/src/{line,shade,symbol,marker}.rs`
- ADR-0025, ADR-0026 (the display list and its renderer), ADR-0008 (crate edges).

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests and the harness decide it, and a wrong result costs one revert.
Stop if a reference image moves.
