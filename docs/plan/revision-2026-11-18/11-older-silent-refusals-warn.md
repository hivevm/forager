# Ticket: The renderer's older silent refusals warn on the layer that drew them

## What to build

ADR-draft-third-party-failures-guarded has every refusal warn, by the maintainer's answer of
2026-11-18, the refusals written before it included. These refusals of `render` and `render-agg`
are silent today:

- **The clip** of `crates/render/src/clip.rs`, ADR-0027's, which every renderer clips through:
  - `line` and `ring` drop the points that are no finite number (about lines 224 and 257);
  - `segment` drops a segment whose coordinate difference overflows (about line 166);
  - `segment` and `ring` drop a crossing that is not finite (about lines 214 and 279).

  A plain stroked line or a filled polygon with such a vertex is drawn through the points left,
  with no warning.
  `Hatch::is_bounded` measures through the same clip; only a renderer's clip call warns, not
  that bound's measurement.
- **A brushed line's points** that are no finite number, which `crates/render-agg/src/brush.rs`
  drops before it turns them into subpixels (about line 452).
- **A TrueType marker's glyph** with a point past `within_glyph_reach`, of which
  `crates/render-agg/src/glyphs.rs` draws nothing (about line 48). `symbolize`'s `glyph_marker.rs`
  does not test the reach. A label's glyph past the reach is cut earlier, by
  `crates/label/src/layout.rs`, and the engine already warns `label-glyph-out-of-reach`
  (`crates/engine/src/labels.rs`). That warning stays the label's one warning: the renderer does
  not warn a second time for it.
- **The skips ticket 03 classifies as guards:** a stroke of no positive width (`render-agg`'s
  `lib.rs`, about line 213), a glyph outline whose width is no finite number (`glyphs.rs`, about
  line 64), a hatch of no positive width (`render-agg`'s `lib.rs`, about line 286), and a hatch's
  run of no positive or no finite length (`render-agg`'s `hatch.rs`, about line 106). One ticket
  03 classifies as a rule of the display list takes no warning.

Each refusal reaches the diagnostics of the layer that drew it as a warning, once per layer and
kind of refusal, through the channel ticket 02 builds from `render-agg` to the layer. The code of
each is agreed with the maintainer in the ticket's first step: `symbol-not-drawn`,
`pixmap-not-drawn`, or one of its own, such as one for the clip's drops.

## Acceptance criteria

- [ ] Given a map whose plain line has a vertex that is no finite number in pixels, when the
  engine renders it, then the layer carries the clip's warning, once. The same holds for a filled
  polygon with such a vertex. The tests live in `crates/engine` and cite the ADR.
- [ ] Given a map whose line has two vertices whose difference overflows, when the engine renders
  it, then the layer carries the clip's warning, once.
- [ ] Given a map whose brushed line has a vertex that is no finite number, when the engine
  renders it, then the layer carries the warning, once.
- [ ] Given a map with a TrueType marker whose glyph has a point past `MAX_GLYPH_REACH`, when the
  engine renders it, then the layer carries the warning, once.
- [ ] Given a map with a label whose glyph lies past `MAX_GLYPH_REACH`, when the engine renders
  it, then the layer carries `label-glyph-out-of-reach` once and no other warning for it.
- [ ] Given each skip ticket 03 classifies as a guard, when an input reaches it, then the layer
  carries its warning once. For each it classifies otherwise, the hand-off says why no warning is
  due.
- [ ] Given the msautotest harness and the engine's tests, when they run, then no case that drew
  without a warning before carries one now.

## Read first

- Touches: `crates/render/src/clip.rs`, `crates/render-agg/src/{brush,glyphs,lib,hatch}.rs`, the
  channel of ticket 02 in `crates/render/src/lib.rs`, `crates/engine/src/layer.rs`, tests under
  `crates/engine/tests/`
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0025, ADR-0027, ADR-0031, ADR-0044,
  ADR-0046; the warning `label-glyph-out-of-reach` in `crates/engine/src/labels.rs`.

## Blocked by

- docs/plan/revision-2026-11-18/02-brushed-piece-refused-once-and-reported.md (the channel)
- docs/plan/revision-2026-11-18/03-guards-marked-and-checked-against-the-lock.md (its class)

## Mode

with the human: it changes the diagnostics a map reports and uses a channel of the renderer
trait, an interface every renderer implements, and it implements a proposed ADR. It may exceed
one session; the clip's drops and the renderer's refusals may land as two pull requests. Stop
when a refusal's warning would fire on a case of the msautotest harness; hand off with the case.
Stop too when reporting the clip's drops needs a public item of `clip` that ADR-0027's *Public
items* clause does not list; that is an ADR.
