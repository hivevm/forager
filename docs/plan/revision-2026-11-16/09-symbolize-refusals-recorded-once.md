# Ticket: What a layer left undrawn is recorded where it is refused, and read in one value

## What to build

What `symbolize` leaves undrawn is held in four places, which the engine reads through four
calls (`take_refusals`, `markers_spent`, `style_trees_over_limit`, `take_style_tree_failure`,
`crates/engine/src/lib.rs`):

- the canvas keeps three maps and a counter of refused hatches;
- the layer's transform keeps a tree's failure and its limit (`layer_transform.rs`);
- the marker budget keeps whether it is spent.

`shade` returns a `bool`, and five callers add it to `canvas.hatching.refused` by hand: `draw.rs`
three times, `circle.rs`, and `label.rs`.

`shade` records its own refused hatch. `Refusals`, the value that `layer.rs` of the engine turns
into the layer's diagnostics, takes in the spent markers and the trees past their limit. The engine
reads it with one call per layer. The diagnostics and their order do not change. This is a
`refactor`.

## Acceptance criteria

- [ ] Given the tree, when `grep -rn "hatching.refused +=" crates/symbolize/src` runs, then it
  finds only `shade`.
- [ ] Given the engine, when a layer is drawn, then it reads what was left undrawn by one call.
- [ ] Given every test that names a diagnostic of a refusal and the msautotest harness, when they
  run, then all pass unchanged, and `scripts/sensor-diagnostics-untested.sh` reports nothing.

## Read first

- Touches: `crates/symbolize/src/{canvas,shade,draw,circle,label,layer_transform}.rs`,
  `crates/engine/src/{lib,layer}.rs`
- ADR-0048 (the warnings of a style's tree); `docs/SPECIFICATION.md` Q-9 (a bound reports once);
  `docs/CONVENTIONS.md` (a bound of Q-9 reports once).

## Blocked by

- docs/plan/revision-2026-11-16/06-engine-draw-layer-and-circle.md (both change a layer's draw)

## Mode

unattended: the tests of the diagnostics decide it, and a wrong result costs one revert. Stop if
a diagnostic's code, text, or order has to change.
