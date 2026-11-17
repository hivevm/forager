# Ticket: msDrawShape's clip is a module of its own, not a part of the hatch

## What to build

The clip that `msDrawShape` applies to every line and polygon lives in `hatch.rs` under the hatch's
name. The struct `Hatching` holds the class's clip buffers, `clip_buffer` and `line_clip_buffer`,
and whether the clip runs in pixels, `clip_in_pixels`. `hatch.rs` also holds `line_paths`, the clip
of every line. `draw.rs` calls `hatch::line_paths`, and the brushed line's reach in `line.rs`
reads `canvas.hatching.line_clip_buffer`. Neither is a hatch. `hatch_rings` and `line_paths` are
near-copies: the duplication sensor lists two clones between them. They differ only in the
rectangle clipper and the transform into pixels.

A module `clip.rs` of `symbolize` holds the class's clip buffers, as a struct named for the clip,
and one function of the shape's clip that takes the clipper and the transform. `hatch.rs` keeps
the hatch. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `draw.rs` and `line.rs` are read, then they take the clip and its
  buffers from `clip`, and no field of the canvas that holds them is named for the hatch.
- [ ] Given `hatch_rings` and `line_paths`, when `scripts/sensor-duplication.sh` runs, then it
  lists no clone between them.
- [ ] Given `cargo test --workspace` and the msautotest harness, when they run, then all pass
  unchanged.

## Read first

- Touches: `crates/symbolize/src/{hatch,draw,line,lib}.rs`, a new `crates/symbolize/src/clip.rs`,
  and the table of `crates/arch-test/tests/symbolize_modules.rs`, which gains a row for `clip` and
  the imports of it
- ADR-0042 (the hatch's cut), ADR-0046 (the brushed line's reach); `docs/ARCHITECTURE.md` (the
  `symbolize` section and its order of modules, updated in the same change).

## Blocked by

- None (can start immediately); ticket 08, which it waited for, has landed

## Mode

unattended: the existing tests decide it, it stays in one crate, and a wrong result costs one
revert. The tests that build a `Hatching` change only in how they build the clip's buffers
(`hatch/tests.rs`), and the module table of `symbolize_modules.rs` gains `clip` as the new
module's imports require. Stop if any other test has to change.
