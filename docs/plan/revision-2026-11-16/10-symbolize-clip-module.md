# Ticket: msDrawShape's clip is a module of its own, not a part of the hatch

## What to build

The clip that `msDrawShape` applies to every line and polygon lives in `hatch.rs` under the hatch's
name: `line_clip_buffer` and `clip_in_pixels`, beside `line_paths`. `draw.rs` (`clipped_lines`)
and the brushed line's reach in `line.rs` call it, though neither is a hatch. `hatch_rings` and
`line_paths` are near-copies. They differ only in the rectangle clipper and the transform into
pixels.

A module `clip.rs` of `symbolize` holds the class's clip buffers and one function of the shape's
clip that takes the clipper. `hatch.rs` keeps the hatch. This is a `refactor`.

## Acceptance criteria

- [ ] Given the tree, when `draw.rs` and `line.rs` are read, then they import the clip from
  `clip`, not from `hatch`.
- [ ] Given `hatch_rings` and `line_paths`, when `scripts/sensor-duplication.sh` runs, then it
  lists no clone between them.
- [ ] Given `cargo test --workspace` and the msautotest harness, when they run, then all pass
  unchanged.

## Read first

- Touches: `crates/symbolize/src/{hatch,draw,line,lib}.rs`, a new `crates/symbolize/src/clip.rs`
- ADR-0042 (the hatch's cut), ADR-0046 (the brushed line's reach); `docs/ARCHITECTURE.md` (the
  `symbolize` section and its order of modules, updated in the same change).

## Blocked by

- docs/plan/revision-2026-11-16/08-symbolize-one-tile-call.md (both change `line.rs`)

## Mode

unattended: the existing tests decide it, it stays in one crate, and a wrong result costs one
revert. Stop if a test has to change.
