# Ticket: The GEOS functions of the layer's transform share one preamble

## What to build

`simplify`, `simplify_preserving`, and `buffer` of `crates/symbolize/src/layer_transform.rs`
open with the same three steps:

- the empty test (`is_empty_from_geos`);
- the port of msGEOSShape2Geometry (`built`);
- the check of the number argument, a tolerance below 0 or a distance that is not finite.

Each step is written three times, with the function's name in the message. The conversion to
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
- ADR-0041, ADR-0047, ADR-0048; `docs/ARCHITECTURE.md` (the `symbolize` section).

## Blocked by

- docs/plan/revision-2026-11-16/11-c-helpers-ported-once.md (both change `layer_transform.rs`)

## Mode

unattended: the existing tests decide it, it stays in one crate, and a wrong result costs one
revert. Stop if a test has to change.
