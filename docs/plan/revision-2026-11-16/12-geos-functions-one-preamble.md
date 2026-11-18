# Ticket: The GEOS functions of the layer's transform share one preamble

## What to build

`simplify`, `simplify_preserving`, and `buffer` of `crates/symbolize/src/layer_transform.rs`
open with the same three steps:

- the empty test (`is_empty_from_geos`);
- the port of msGEOSShape2Geometry (`built`);
- the check of the number argument, a tolerance below 0 or a distance that is not finite.

Each step is written three times, with the function's name in the message. Ticket 20 added a
fourth to two of them, through one helper, `is_finite`, with the function's name in the message:

- `buffer` refuses any shape with a coordinate that is no finite number;
- `simplify` refuses only a polygon with one (`layer_transform.rs`, its `Kind::Polygon` arm). A
  line and a point keep their simplify, which
  `simplify_keeps_a_line_or_a_point_with_a_coordinate_that_is_no_number` pins;
- `simplifypt` refuses none.

The conversion to geo's types already lives in `geometry.rs` (`to_polygon`, `from_polygon`). The
steps in front of it stay in `layer_transform.rs`, 953 lines, beside the tree's evaluation and the
ports of `generalize`, `densify`, `smoothsia`, `outer`, and `inner`.

One function takes the shape, the function's name, and the argument's check, and gives the shape
back or its `Areas`. The three GEOS functions call it. The finiteness check is part of what each
function passes it, so that each keeps the refusals above. `built` and `areas` stay in
`layer_transform.rs`, as the maintainer decided; the finiteness check stays there too, a guard of
ADR-0049 in the crate that calls geo. The messages do not change.
This is a `refactor`. (Amended by the design revision of 2026-11-18 for ticket 20's check.)

## Acceptance criteria

- [ ] Given the tree, when `layer_transform.rs` is read, then `is_empty_from_geos` and `built`
  are each called once on the way to `simplify`, `simplifypt` (`simplify_preserving`), and
  `buffer`.
- [ ] Given `crates/symbolize/src/geometry/tests.rs`, `layer_transform/tests.rs`, and the
  msautotest harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/symbolize/src/layer_transform.rs`,
  `crates/symbolize/src/layer_transform/tests.rs` (corrected 2026-11-18: the preamble stays in
  `layer_transform.rs`, so `geometry.rs` is not touched; the tests that pin the preamble's order
  are added beside the others)
- ADR-0041, ADR-0047, ADR-0048; `docs/adr/0049-third-party-failures-guarded.md`;
  `docs/ARCHITECTURE.md` (the `symbolize` section).

## Blocked by

- docs/plan/revision-2026-11-16/11-c-helpers-ported-once.md (both change `layer_transform.rs`)

## Mode

with the human: `built` charges the transform's budget of points (Q-9), and ADR-0006 keeps the
resource limits from unattended work.
