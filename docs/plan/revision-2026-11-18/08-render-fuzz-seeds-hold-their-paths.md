# Ticket: Each seed of the render fuzz target is held to the path it was made for

## What to build

The render target (`fuzz/fuzz_targets/render.rs`) reads six views over overlapping slices of the
same bytes: `Input` forward, `Styles` and `Cache` from the reversed bytes, `Expressions` from the
middle, `TransformInput` from the last third, and `Fidelity` from the first third, reversed. A
field added to any view, or a variant added before others, as `StepInput`'s `Simplify`,
`Simplifypt`, and `Buffer` were, changes what every older seed decodes to. Three seeds lost the
path they were made for this way and went unnoticed, and the target's own header says the
seeds "now … render" differently. `docs/backlog/render-seeds-lost-their-paths.md` describes it,
and this ticket deletes that entry.

A mistake a check could have caught becomes a check
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)):

- the target's decoding becomes a function the fuzz workspace can call from a test, without
  `fuzz_target!`;
- a test of the fuzz workspace decodes each seed under `fuzz/seeds/render/` and asserts that it
  builds what its name says it was made for: a TrueType symbol, a circle layer, a step of the
  transform tree, a tiled pixmap, a far segment;
- a seed with no entry fails the test, so a new seed names its path;
- the three lost seeds, `far-end-rounds-to-its-parameter`, `far-end-rounds-to-its-parameter-slow`,
  and `pixmap-tiled-fill`, are re-derived by a short run against a temporary panic at the path,
  not committed;
- `scripts/check-rust.sh`, or the script that already builds the fuzz workspace, runs the test.

Once the test holds, the clause of `docs/CONVENTIONS.md`'s fuzz seed entry that asks a decoding
change to re-run each seed is dropped: the check decides it.

## Acceptance criteria

- [ ] Given a seed whose decoding no longer builds its path, when the test runs, then it fails and
  names the seed and the path.
- [ ] Given a seed under `fuzz/seeds/render/` with no entry in the test, when it runs, then it
  fails and names the seed.
- [ ] Given the tree after this ticket, when the test runs, then every seed passes, the three
  re-derived ones among them.
- [ ] Given `scripts/check-all.sh`, when it runs, then it runs the test.

## Read first

- Touches: `fuzz/fuzz_targets/render.rs`, a test of the fuzz workspace (new), `fuzz/Cargo.toml`,
  `fuzz/seeds/render/`, `scripts/check-rust.sh`, `docs/CONVENTIONS.md` (the fuzz seed entry),
  `docs/backlog/render-seeds-lost-their-paths.md` (deleted)
- Q-3; ADR-0007; `scripts/check-rust.sh` (it lints the fuzz workspace with the pinned toolchain,
  lines 158-161); the commit message of e265e323.

## Blocked by

- docs/plan/revision-2026-11-16/02-check-transform-variants-fuzzed.md (it reads the decoding)

## Mode

unattended: the test decides it, and a wrong result costs one revert. It touches no product
crate. Stop when a path can be told only by code inside a product crate, such as a counter in
`render-agg`; hand off with the paths that need one. Stop too after one hour of fuzzing for the
three seeds altogether, and hand off with the seed not found.
