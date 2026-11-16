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

## Where it stands (2026-11-16, stopped at the Mode's stop)

- Built: `fuzz/fuzz_targets/render.rs` reads `simplify`, `simplifypt`, and `buffer` steps in
  `StepInput`. It also reads a `Fidelity` from the first third of the bytes, back to front: a class
  style's transform tree on the polygon and line layers, the map's `ANGLE`, a projection of the map
  and its layers, a circle layer, `ANTIALIAS FALSE` on the first style, and a TrueType symbol at
  points, along lines, and over polygons. Its font is the test font, or bytes the fuzz input gives.
  `docs/ARCHITECTURE.md` names the new paths.
- Seeds: eight new seeds under `fuzz/seeds/render/`: `transform-simplify`,
  `transform-simplifypt`, `transform-buffer`, `class-geomtransform`, `truetype-marker`,
  `circle-layer`, `map-angle-projected`, and `antialias-false`. Each was found by a short fuzz
  run against a temporary panic at its path, and each was checked to fire it. The panics are not
  committed.
- The first 300-second run from these seeds crashed: a panic in agg-rust's clipper on the
  TrueType marker's outline. It is filed with its input in
  `docs/backlog/truetype-marker-outline-overflows-agg-clipper.md`. The fourth criterion is open
  until that is fixed and the run is clean.
- `scripts/check-all.sh` was not run, because the host's disk was full: about 1 MB was free, and
  the shared `target/` directories took most of the space. The fuzz workspace's `cargo fmt --check`
  and `cargo clippy -D warnings` from `scripts/check-rust.sh` pass, and so does
  `scripts/check-docs.sh`.
- Note: `scripts/fuzz.sh` takes seconds, not a target name, so `scripts/fuzz.sh render` does not
  run one target. The run above used `cargo fuzz run render` with fuzz.sh's flags.
- Held on the maintainer's word (2026-11-16): the crash is fixed first by
  `docs/plan/revision-2026-11-16/16-glyph-outline-width-bounded.md`. This branch then rebases onto
  that fix, adds the crash input as a seed, deletes its backlog entry, runs the 300 seconds again,
  and runs `scripts/check-all.sh`.
