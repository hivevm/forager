# Ticket: Tests for the diagnostic codes no test names

## What to build

Each diagnostic code that the core writes and no test names gets a test. The test pushes the code
through the load or the render that emits it and asserts the code. On 2026-11-15,
`bash scripts/sensor-diagnostics-untested.sh` lists seven:

- `size-missing`, `extent-missing`, `view-invalid`: the engine's view of the map, for a map without
  `SIZE`, without `EXTENT`, and with an extent and size that give no scale. `View::for_image` in
  `crates/symbolize/src/extent.rs` refuses a side of 1 pixel or less, a corner that is not finite,
  and a cellsize that is not finite and positive.
- `layer-not-drawn`: a shown layer whose render mode the engine does not draw, such as a raster or
  annotation layer.
- `extent-not-reprojected`: a layer in another projection, where no point of the map's extent
  reprojects into the layer's projection. An example is an orthographic layer whose hemisphere does
  not contain the extent.
- `yaml-write`, `json-write`: `StyleDocument::to_yaml` and `to_json`, when serialization fails.

A test must assert the code, not only the severity. If a code cannot be reached through a public
load or render, the session must not add a test hook to reach it. It writes why in the hand-off, as
the sensor allows, and files the finding in `docs/backlog/`. `json-write` is the likely case:
`serde_json` writes a non-finite number as `null` and fails only on a non-string map key or a
`Serialize` that errors. The finding states whether the call can fail at all, and if it cannot,
whether the code is dead.

## Acceptance criteria

- [ ] Given a map without `SIZE`, when the engine renders it, then the result holds the error
  `size-missing` and no image; likewise `extent-missing` for a map without `EXTENT`, and
  `view-invalid` for a map of `SIZE 1 1` or an empty extent.
- [ ] Given a shown layer whose render mode is raster, when the engine renders the map, then it
  warns `layer-not-drawn` naming the layer, and the other layers draw as before.
- [ ] Given a layer in a projection that no point of the map's extent reprojects into, when the
  engine renders the map, then it warns `extent-not-reprojected` and asks the layer's source for
  nothing.
- [ ] Given a style document `to_yaml` or `to_json` cannot write, when it is written, then the
  error is `yaml-write` or `json-write`. Where no such document exists, the hand-off says why and a
  backlog entry records it.
- [ ] Given the tree after the change, when `bash scripts/sensor-diagnostics-untested.sh` runs, then
  it lists none of the seven, except those the hand-off gives a reason for.

## Read first

- Touches: `crates/engine/tests/render.rs`, `crates/engine/tests/reproject.rs`,
  `crates/style-ir/src/document.rs` (its `tests` module)
- `crates/engine/src/lib.rs` (`view`, `drawable_mode`, `asked`), `crates/symbolize/src/extent.rs`
  (`View::for_image`), `crates/engine/src/reproject.rs`, `crates/style-ir/src/document.rs`
  (`to_yaml`, `to_json`)
- `scripts/sensor-diagnostics-untested.sh` (its header: what counts as a test that names a code)
- `docs/CONVENTIONS.md` (diagnostic codes)
- Golden path: `an_image_past_the_size_limit_is_an_error` in `crates/engine/tests/render.rs`, and
  the test of `projection-unsupported` in `crates/engine/tests/reproject.rs`
- Seam: the engine's render with sources in memory; the style document's writers

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides each criterion, and the sensor's list confirms the result.
- The scope is tests only. A wrong result costs one revert.
- It touches no part ADR-0006 keeps with the human. It changes no code under `crates/*/src` apart
  from the `tests` module of `document.rs`.
- A wrong result shows as a test that does not assert the code or a sensor that still lists it. The
  reviewer checks each assertion.
- Stop: when a code needs a change to production code to be reached, or after two attempts at
  reaching one, the session records it in the hand-off and the backlog and moves on. When every code
  is tested or recorded, it hands off.
