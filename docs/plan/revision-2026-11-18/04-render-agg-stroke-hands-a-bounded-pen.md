# Ticket: `render-agg` strokes with the pen it has bounded, not a pen and a width beside it

## What to build

Ticket 04 of revision-2026-11-16 made a stroke's pen one value, `Pen`
(`crates/render/src/lib.rs`). In `crates/render-agg/src/lib.rs`, `rasterize` and `apply` now take
`(pen, drawn)`, and their docs say that "the pen's own width is never drawn as it is". Their one
caller, `stroke`, computes `drawn` by `drawn_width` and hands both on. A value the callee must
ignore is a trap for the next caller, the smell of an interface that widened.

The review of that ticket saw the trap, and 8b7cf655 answered it by naming the width `drawn` and
saying in both docs that the pen's own width is never drawn. That documents the trap and keeps
it: a third caller still gets a pen whose width is wrong to use. Since then,
ADR-draft-third-party-failures-guarded names `drawn_width` a guard of agg-rust's fixed-point
coordinates, which stands in one place with its marker. A pen bounded once in `stroke` keeps that
guard in one place on the stroke's path.

The bound of a width is written three ways around `render::clip::drawn_width`:

- `stroke` refuses a width that is no positive number, then bounds it;
- a hatch bounds `width.abs()` (`crates/render/src/hatch.rs`);
- a glyph's outline bounds `abs()` and keeps the sign by `copysign`
  (`crates/render-agg/src/glyphs.rs`).

`stroke` hands on a pen whose width is the drawn one, and `rasterize` and `apply` take that pen
alone. Where the three bounds share one rule, `clip` holds it once, a sign-keeping variant if the
glyph needs one. No pixel moves. This is a `refactor`.

## Acceptance criteria

- [ ] Given `crates/render-agg/src/lib.rs`, when it is read, then `rasterize` and `apply` take no
  width beside the pen.
- [ ] Given the tests of `render`, `render-agg`, and the engine, and the msautotest harness, when
  they run, then all pass, no assertion changed.

## Read first

- Touches: `crates/render-agg/src/lib.rs`, `crates/render-agg/src/glyphs.rs`,
  `crates/render/src/clip.rs`, `crates/render/src/hatch.rs`
- ADR-0025, ADR-0027, ADR-0042 (the hatch's cost model);
  `docs/adr/draft-third-party-failures-guarded.md`; the commit 8b7cf655; `docs/CONVENTIONS.md`
  (a commit typed `refactor`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing suite and the msautotest harness decide it, and a wrong result costs one
revert. It changes no resource limit, but `Hatch::drawn_width` of `hatch.rs` feeds the hatch's
cost model, `Hatch::is_bounded` (ADR-0042), which bounds a hatch against Q-9: the bounded width a
hatch's lines are measured at must stay the same value, and the tests of `is_bounded` pass
unchanged. Stop when a test's expected value would change, or when a
fourth way of bounding a width turns up that the shared rule cannot take; hand off with the
difference named.
