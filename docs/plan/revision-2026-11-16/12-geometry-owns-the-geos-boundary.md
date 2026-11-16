# Ticket: The geometry module owns the conversion to and from the shapes GEOS computes on

## What to build

The boundary between a feature's shape and the polygons that `simplify`, `simplifypt`, and
`buffer` compute on is split between two modules:

- `layer_transform.rs`, 960 lines, holds `built`, `areas`, and `is_empty_from_geos`, beside the
  tree's evaluation and the ports of `generalize`, `densify`, `smoothsia`, `outer`, and `inner`;
- `geometry.rs` holds `to_polygon` and `from_polygon`.

`simplify` and `simplify_preserving` repeat one preamble: the empty check, `built`, and a
tolerance below 0. ADR-0041 and ADR-0047 place the computations in the geometry module, but they
leave the conversion unplaced.

The conversion, the empty test, and the shared preamble move into `geometry.rs`. That module
takes a shape and gives a shape back, its budget charged as now. `layer_transform.rs` keeps the
evaluation of the tree. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `layer_transform.rs` is read, then it calls one function of `geometry`
  for each of `simplify`, `simplifypt`, and `buffer`, and holds no conversion to geo's types.
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
