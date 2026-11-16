# Ticket: The engine's `Rotation` owns the map's angle

## What to build

The map's `ANGLE` reaches the engine's drawing along two ways:

- `Rotation` (`crates/engine/src/lib.rs`, ADR-0045) holds the frame, the geotransform, and the
  geo cellsize, but not the angle;
- `Drawing` carries `map.angle` again as `map_angle` and turns a style's angle with it
  (`Drawing::turned`). The labels turn theirs from `self.map.angle` (`labels.rs`, `turned`).
  Both go through `turned_by_map` of `layer.rs`.

Zero is filtered three times: by the frontend, by `Rotation::of`, and by `Drawing::turned`. The
geo cellsize lives in `classes.rs` (`rotated_geo_cellsize`).

The concept is one: the map's rotation. `Rotation` keeps the angle it was built from. It answers
the turned angle of a style and of a label, by the render mode that MapServer's
`*LayerDrawShape` gives. `Drawing` and the labels take it in place of `map.angle`. The zero
check stays in the frontend and in `Rotation::of`. This is a `refactor`: nothing a test sees
changes.

## Acceptance criteria

- [ ] Given the tree, when `grep -n "map\.angle" crates/engine/src` runs, then it finds only
  `Rotation::of`, and `Drawing` holds no `map_angle`.
- [ ] Given `crates/engine/tests/map_angle.rs` and the msautotest cases of `ANGLE`, when
  `cargo test --workspace` runs, then all pass unchanged and no reference image is re-blessed.
- [ ] Given the commit, when `scripts/sensor-refactor-tests.sh` runs, then it lists no test the
  refactor changed.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/layer.rs`, `crates/engine/src/classes.rs`
- ADR-0045; `docs/CONVENTIONS.md` (a `refactor` changes nothing a test can see).

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests decide it, it stays in one crate, and a wrong result costs one
revert. Stop if a test has to change. That would be a change of behaviour and is for the human.
