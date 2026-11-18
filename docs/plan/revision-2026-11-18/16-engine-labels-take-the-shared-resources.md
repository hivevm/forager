# Ticket: The labels take a render's resources in one shape

## What to build

Ticket 05 of revision-2026-11-16 put a render's resources in one place,
`crates/engine/src/render_resources.rs` (merge 6f3816ea). Ticket 13 handed the canvas and the label
cache one `SymbolResources` (merge 9ce92326). Their reviews named four seams that stayed uneven:

- `Labels::collect` and `Labels::label_at_once` take `&mut Shared`, while
  `Labels::layer_at_once` and `Labels::finish` take `&Pixmaps` (`crates/engine/src/labels.rs`),
  because those two lay out no text;
- `Shared::symbol_resources()` builds the value once per layer for the canvas
  (`crates/engine/src/lib.rs`) and again for each label laid out (`lay_out`, `labels.rs`), from the
  same fields. It holds references only, so the copies cost nothing measurable; the seam is that
  the canvas and the labels each build what is one value;
- the loader is held twice, as `Resources::loader` and inside `Fonts`;
- `load_resources` both loads and lends, since splitting it put `render_with_files` past clippy's
  100 lines at the time. Ticket 06 has since moved the drawing of a layer into `draw_layer`.

The labels take the render's resources in one shape. Whether the symbol resources are built once
for a layer and its labels, whether the loader is held once, and whether loading and lending
split, the session decides by what the change shows: each is done only where it makes the code
smaller, and the hand-off says which and why. No count, refusal, bound, or diagnostic of Q-9
moves, and one font store still serves the labels and the TrueType markers.

## Acceptance criteria

- [ ] Given `crates/engine/src/labels.rs`, when its methods are read, then each that needs the
  render's images or fonts takes them in the same shape, and none takes `&Pixmaps` beside it.
- [ ] Given every test of the workspace, the pin of 992572b2 (one font store for labels and
  markers) among them, when they run, then all pass, no assertion changed.

## Read first

- Touches: `crates/engine/src/render_resources.rs`, `crates/engine/src/labels.rs`,
  `crates/engine/src/lib.rs`, `crates/engine/src/fonts.rs`, `crates/engine/src/label_styles.rs`
- The merge messages of 6f3816ea and 9ce92326, their *Doubts* and *Review*; ADR-0029, ADR-0044;
  the convention *A commit typed `refactor` changes nothing a test can see*.

## Blocked by

- docs/plan/revision-2026-11-18/14-engine-layer-reports-one-module.md

## Mode

unattended: the existing tests decide it, the change preserves every interface outside the crate,
and a wrong result costs one revert. Stop when one shape for the labels needs a lifetime or a
borrow that clippy or the borrow checker refuses after two attempts; hand off with the attempts.
