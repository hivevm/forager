# Ticket: The engine draws a layer in a function of its own, a circle layer's case in a module

## What to build

`render_with_files` (`crates/engine/src/lib.rs`) holds a loop body of about 115 lines:

- `at_once` splits it into two inline branches;
- `Drawing::new` takes six positional parameters;
- `load_resources` takes a `cached: bool`;
- `reproject_features` only forwards to `reproject_features_within`.

A circle layer is a special case at five places:

- `in_view` returns an `Option<LayerReprojection>` that only a circle layer uses;
- `Drawing.centres` holds it;
- the mode is tested at three more places of `lib.rs`, and once in `labels.rs`.

The body of the loop becomes `draw_layer`. A point layer drawn at once, labels with its markers,
gets a function of its own in place of the flag. `load_resources` loses its flag for a second
function or for the caller's choice. The circle layer's centres, their reprojection, and the
shape of two points they come from move into a module of the engine. The other modes do not see
the circle layer's reprojection. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `render_with_files` is read, then its loop calls `draw_layer`, and no
  function of the engine takes a `bool` to choose between two ways of drawing.
- [ ] Given the tree, when `grep -n "RenderMode::Circle" crates/engine/src` runs, then it finds
  only the dispatch and the new module.
- [ ] Given `crates/engine/tests/circle.rs`, `cargo test --workspace`, and the msautotest
  harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/reproject.rs`, a new `crates/engine/src/circle.rs`
- ADR-0037, ADR-0045 (the reprojection the circle's centres take);
  `docs/semantics/` (the page of the circle layer); `docs/ARCHITECTURE.md` (the engine section).

## Blocked by

- docs/plan/revision-2026-11-16/05-engine-render-resources.md (both change the render loop)

## Mode

with the human: it moves `reproject_features_within`, which holds Q-9's bound on the horizon
points (`MAX_HORIZON_POINTS`), and ADR-0006 keeps the resource limits from unattended work.
