# Ticket: The GEOS functions of the layer's transform share one preamble

## What to build

`simplify`, `simplify_preserving`, and `buffer` of `crates/symbolize/src/layer_transform.rs`
open with the same three steps:

- the empty test (`is_empty_from_geos`);
- the port of msGEOSShape2Geometry (`built`);
- the check of the number argument, a tolerance below 0 or a distance that is not finite.

Each step is written three times, with the function's name in the message. Ticket 20 added a
fourth to two of them: `simplify` of a polygon and `buffer` refuse a shape with a coordinate that
is no finite number, through one helper, `is_finite`, with the function's name in the message.
`simplifypt` does not refuse one. The preamble takes that check too, for `simplify` and `buffer`
alone, so that `simplifypt` keeps its behaviour. Where the check lives, here or in
`geometry.rs`, follows ADR-draft-third-party-failures-guarded once the maintainer decides it;
until then it stays in `layer_transform.rs`. (Amended by the design revision of 2026-11-18.) The
conversion to
geo's types already lives in `geometry.rs` (`to_polygon`, `from_polygon`). The steps in front of
it stay in `layer_transform.rs`, 960 lines, beside the tree's evaluation and the ports of
`generalize`, `densify`, `smoothsia`, `outer`, and `inner`.

One function takes the shape, the function's name, and the argument's check, and gives the shape
back or its `Areas`. The three GEOS functions call it. Whether `built` and `areas` move to
`geometry.rs` beside the conversion is decided in the ticket, against ADR-0041 and ADR-0047,
which place the computations there. The messages do not change. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `layer_transform.rs` is read, then `is_empty_from_geos` and `built`
  are each called once on the way to `simplify`, `simplifypt`, and `buffer`.
- [ ] Given `crates/symbolize/src/geometry/tests.rs`, `layer_transform/tests.rs`, and the
  msautotest harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/symbolize/src/{layer_transform,geometry}.rs`
- ADR-0041, ADR-0047, ADR-0048; `docs/adr/draft-third-party-failures-guarded.md`;
  `docs/ARCHITECTURE.md` (the `symbolize` section).

## Blocked by

- docs/plan/revision-2026-11-16/11-c-helpers-ported-once.md (both change `layer_transform.rs`)

## Mode

with the human: `built` charges the transform's budget of points (Q-9), and ADR-0006 keeps the
resource limits from unattended work.
