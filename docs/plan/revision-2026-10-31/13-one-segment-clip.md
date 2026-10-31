# Ticket: Clip a segment once, in `render`, for the hatch's bound and the renderer

## What to build

`visible_length` in `crates/render/src/lib.rs` and `past` and `clip_segment` in
`crates/render-agg/src/clip.rs` are two Liang–Barsky clippers, each with its own precision fix for
measuring from the far end; the margin `width.abs()/2 + 2.0` of `Hatch::is_bounded` repeats
`CLIP_MARGIN + width/2` of the renderer as a literal, and only the renderer clamps the width first.
Give `render` one segment clip and one named margin that `is_bounded` and `render-agg` share. The
cost model of the hatch, about 330 of `render`'s 617 lines, moves into `render/src/hatch.rs` on the
way ([ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md)).

## Acceptance criteria

- [ ] Given the clipping tests of `render-agg` and the bound tests of `Hatch`, when they run against
  the one clip, then they pass unchanged.
- [ ] Given a hatch whose width is past the renderer's clamp, when `is_bounded` judges it, then it
  uses the width the renderer draws.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md); `crates/render/src/lib.rs`;
  `crates/render-agg/src/clip.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing clip and hatch tests decides its criteria; its scope is `render` and
`render-agg`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a moved reference image or a failing clip test; stop: after two attempts
that leave `scripts/check-all.sh` red, or when a reference image moves, the session leaves its state
in this file and hands off.
