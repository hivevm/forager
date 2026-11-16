# Ticket: The render fuzz target reaches the paths the fidelity spec added

## What to build

The fidelity spec added paths of the renderer that the render fuzz target does not build
(`fuzz/fuzz_targets/render.rs`): a layer transform's `simplify`, `simplifypt`, and `buffer`
steps, which run ADR-0041's Douglas-Peucker, ADR-0047's port of JTS's topology-preserving
simplifier, and geo's buffer; a class style's `GEOMTRANSFORM` expression (ADR-0048); a TrueType
marker drawn through the glyph port (ADR-0044); a circle layer; a style's `ANTIALIAS FALSE`; and
a map's `ANGLE` (ADR-0045). The proj target already reaches the rotation's geotransform. The
convention "A change that gives a target of Q-3 a new reader or a new path brings the fuzz seed
that reaches it" held for the brushed line only (ebc0a4c5). The simplifypt port keeps its own
stack and must stop on a section of no number (ADR-0047), so a hang there is a Q-3 failure.

The target's input gains these choices, and its seeds reach each of them.

## Acceptance criteria

- [ ] Given the render target, when its `StepInput` is read, then it can build every variant of
  `TransformShape`, `Simplify`, `Simplifypt`, and `Buffer` among them.
- [ ] Given the render target, when a class style is built, then it may carry a transform tree of
  the same steps.
- [ ] Given the render target, when its map is built, then it may carry an `ANGLE`, a circle
  layer, a style with `ANTIALIAS FALSE`, and a TrueType symbol whose font the fuzz input or a
  seed font supplies.
- [ ] Given `fuzz/seeds/render/`, when `scripts/fuzz.sh render` runs for the time Q-3 gives a
  target, then it ends without a panic, a crash, a hang past 10 s, or more than 2 GiB, and a new
  seed for each path above is in the tree.

## Read first

- Touches: `fuzz/fuzz_targets/render.rs`, `fuzz/seeds/render/`
- `docs/CONVENTIONS.md` (the fuzz seed convention); `docs/SPECIFICATION.md` Q-3;
  ADR-0041, ADR-0044, ADR-0045, ADR-0047, ADR-0048.
- The golden path: the brushed line's addition to the target in ebc0a4c5.

## Blocked by

- None (can start immediately)

## Mode

unattended: the fuzz run decides it, and the change touches only `fuzz/`, so a wrong result costs
one revert. A crash the run finds is not fixed here: it is filed in `docs/backlog/` with its
input, and the session stops.
