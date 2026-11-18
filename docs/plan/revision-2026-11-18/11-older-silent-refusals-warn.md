# Ticket: The renderer's older silent refusals warn on the layer that drew them

## What to build

ADR-draft-third-party-failures-guarded has every refusal warn, by the maintainer's answer of
2026-11-18, the refusals written before it included. Three of `render-agg`'s refusals are silent
today:

- `crates/render-agg/src/brush.rs` (about line 452) drops the points of a brushed line that are no
  finite number before it turns them into subpixels. The line is drawn through the points left;
- `crates/render-agg/src/glyphs.rs` (about line 48) draws nothing of a glyph with a point past
  `within_glyph_reach` of `crates/render/src/clip.rs`;
- `crates/render-agg/src/lib.rs` (about line 213) strokes nothing of a pen whose width is no
  positive number. Whether it guards agg-rust is ticket 03's classification. MapServer replaces a
  width of 0 or below by the scale factor before it draws (`maprendering.c:797`, `:808`), so the
  ticket also says whether such a pen can reach `stroke` from `symbolize` at all.

Each refusal that is a guard reaches the diagnostics of the layer that drew it as a warning, once
per layer, through the channel ticket 02 builds from `render-agg` to the layer. The code of each is
`symbol-not-drawn`, `pixmap-not-drawn`, or one the maintainer names, agreed in the ticket's first
step. A glyph of a label warns as the label's layer.

## Acceptance criteria

- [ ] Given a map whose brushed line has a point that is no finite number, when the engine renders
  it, then the layer carries the warning, once. The test lives in `crates/engine` and cites the
  ADR.
- [ ] Given a map with a glyph past `MAX_GLYPH_REACH`, when the engine renders it, then the layer
  carries the warning, once.
- [ ] Given ticket 03's classification of the stroke of no positive width as a guard, when a pen
  of width NaN reaches `stroke`, then the layer carries the warning. Given the other
  classification, the hand-off says why no warning is due.
- [ ] Given the msautotest harness and the engine's tests, when they run, then no case that drew
  without a warning before carries one now.

## Read first

- Touches: `crates/render-agg/src/{brush,glyphs,lib}.rs`, the channel of ticket 02 in
  `crates/render/src/lib.rs`, `crates/engine/src/layer.rs`, tests under `crates/engine/tests/`
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0025, ADR-0027, ADR-0031, ADR-0046;
  `src/maprendering.c` of MapServer at the pinned release, lines 797 and 808 (the width of 0 or
  below).

## Blocked by

- docs/plan/revision-2026-11-18/02-brushed-piece-refused-once-and-reported.md (the channel)
- docs/plan/revision-2026-11-18/03-guards-marked-and-checked-against-the-lock.md (its class)

## Mode

with the human: it changes the diagnostics a map reports and uses a channel of the renderer
trait, an interface every renderer implements, and it implements a proposed ADR. Stop when a
refusal's warning would fire on a case of the msautotest harness; hand off with the case.
