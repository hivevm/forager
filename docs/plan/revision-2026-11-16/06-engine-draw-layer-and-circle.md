# Ticket: The engine draws a layer in a function of its own, a circle layer's case in a module

## What to build

`render_with_files` (`crates/engine/src/lib.rs`) holds a loop body of about 85 lines
(checked against the tree on 2026-11-18, after revision-2026-11-16/05 and revision-2026-11-18/02
and /11):

- `at_once` splits it into two inline branches;
- `Drawing::new` takes five positional parameters;
- `load_resources` takes a `cached: bool`, and returns the `Lent` of
  `crates/engine/src/render_resources.rs`, since ticket 05 could not split loading from lending
  within clippy's 100 lines of `render_with_files`;
- `reproject_features` only forwards to `reproject_features_within`.

A circle layer is a special case at five places:

- `in_view` returns an `Option<LayerReprojection>` that only a circle layer uses;
- `Drawing.centres` holds it;
- the mode is tested at three more places of `lib.rs` (`style_reports`, `drawable`,
  `Drawing::draw`), and once in `labels.rs` (`settle_layer`). The list of the modes drawn,
  `drawn`, names it as it names the others, and is no special case.

The body of the loop becomes `draw_layer`. A point layer drawn at once, labels with its markers,
gets a function of its own in place of the flag. `load_resources` loses its flag for a second
function or for the caller's choice, and loads without lending, the layer's function lending
after it. The circle layer's centres, their reprojection, and the engine's use of the shape of
two points they come from move into a module of the engine; that shape itself, `circle` and
`circle_ops`, stays in `crates/symbolize/src/circle.rs`, a crate of its own. The other modes do
not see the circle layer's reprojection. The reprojection of a layer's features with its
warnings moves into `reproject.rs`. The renderer's `take_refused` after each layer and the
refusals `Labels::finish` takes of the cache stay where they are. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `render_with_files` is read, then its loop calls `draw_layer`, and no
  function of the engine takes a `bool` to choose between two ways of drawing.
- [ ] Given the tree, when `grep -n "RenderMode::Circle" crates/engine/src` runs, then it finds
  only the dispatch and the new module.
- [ ] Given `crates/engine/tests/circle.rs`, `cargo test --workspace`, and the msautotest
  harness, when they run, then all pass unchanged.

## Read first

- Touches: `crates/engine/src/lib.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/reproject.rs`, a new `crates/engine/src/circle.rs`, the table of
  `crates/arch-test/tests/engine_modules.rs`, the engine section of `docs/ARCHITECTURE.md`
- ADR-0037, ADR-0045 (the reprojection the circle's centres take);
  `docs/semantics/` (the page of the circle layer); `docs/ARCHITECTURE.md` (the engine section).

## Blocked by

- docs/plan/revision-2026-11-16/05-engine-render-resources.md (both change the render loop)

## Mode

with the human: it moves `reproject_features_within`, which holds Q-9's bound on the horizon
points (`MAX_HORIZON_POINTS`), and ADR-0006 keeps the resource limits from unattended work.
