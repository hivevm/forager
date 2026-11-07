# Ticket: Lay a hatch's lines inside `render`'s grown image, with the canvas's one image size

## What to build

[ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md) puts the image grown by
its margin in `crates/render/src/clip.rs`, with `Rect::around_image` public for every user. Two
copies remain in `crates/symbolize`:

- `hatch.rs` lays a hatch's lines over the image grown by `width + 2.0` of its own: the `2.0` is
  `clip.rs`'s `CLIP_MARGIN` written again, and the grow is the whole width where `clip` grows by
  half the width drawn. It is safe while the area is the larger, and drifts the day either moves.
- The image's size is held twice: `Canvas` holds it as the renderer's `u32`, and
  `View::image_size` rebuilds it from the extent and the cellsize with a rounding; `shade` hands the
  rebuilt size to the hatch, which casts it back to `u32` for `Hatch::is_bounded`, while the
  renderer checks the same bound with the true size, and line markers and `GEOMTRANSFORM` read the
  canvas's.

Lay the hatch's lines inside `Rect::around_image` at the width drawn, hand the hatch the canvas's
size, and drop `View::image_size` if nothing else needs it.

## Acceptance criteria

- [ ] Given a hatched polygon across the image's edge, when it renders, then its pixels are those of
  today's tests and the conformance outcome of the hatch cases does not move.
- [ ] Given an extent whose span over the cellsize rounds to a size other than the image's, when a
  hatch is bounded, then the bound is measured on the image's own size, with a test that states
  both sizes.
- [ ] Given `crates/symbolize`, when it is searched, then no margin of the image is written outside
  `render`'s clip.

## Read first

- [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md),
  [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md); `crates/render/src/clip.rs`,
  `crates/symbolize/src/hatch.rs`, `crates/symbolize/src/extent.rs` (`image_size`),
  `crates/symbolize/src/draw.rs` (`shade`, `Canvas`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the hatch tests, the conformance list, and the new size test decide it; its scope is
two modules of `symbolize`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; stop: when a pixel of a hatch test moves, the session records the difference here and hands
off instead of re-blessing.
