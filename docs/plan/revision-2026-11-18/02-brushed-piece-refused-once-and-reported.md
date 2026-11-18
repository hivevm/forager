# Ticket: A brushed line's piece past agg-rust's `i32` is refused by one probe and warned on its layer

## What to build

`render-agg` refuses a piece of a brushed line in two steps (`crates/render-agg/src/brush.rs`):

- `within_i32` bounds the pattern's place by the longest segment, which `filtered` ports AGG's
  vertex filter again to find;
- `JoinProbe` runs agg-rust's outline rasterizer once without pixels and bounds the joins.

The probe's `line3` already receives each kept segment's `lp.len`, the length `within_i32` takes
from `filtered`. Each piece is walked three times, and both steps refuse silently. A user sees a
line with a gap and no diagnostic.

As ADR-draft-third-party-failures-guarded decides, with the maintainer's answer of 2026-11-18
that a refused piece warns on its layer:

- the probe takes the place's bound too, so that one guard and one pass before the draw remain,
  and `filtered` goes if nothing else uses it;
- a refused piece reaches the diagnostics of the layer that draws it as a warning, once per
  layer and symbol, as a refused tile warns (`crates/engine/src/layer.rs`). The code is
  `symbol-not-drawn` or `pixmap-not-drawn` by the symbol's kind, as the maintainer chose on
  2026-11-18;
- the guard carries the marker `// Guard: agg-rust 1.2.0`, and its doc comment names the lines
  it was derived from.

The renderer trait of `crates/render` returns nothing from `draw` today, so the channel from
`render-agg` to the layer's diagnostics is new. It changes the trait every renderer implements.
Its shape is the maintainer's choice of 2026-11-18: `draw` stays as it is, the renderer counts its
refusals by kind, and the engine takes them after each layer through one new method of the trait
and turns each kind into one warning. It may exceed one session: the channel and the probe's merge
are two commits that may land as two pull requests.

## Acceptance criteria

- [ ] Given the hairpin of `brush/tests.rs` at `HAIRPIN_RUN` and one subpixel past it, when each
  is drawn, then the first draws as today and the second draws nothing.
- [ ] Given a map whose layer brushes the fuzz path of
  `a_piece_whose_join_lies_far_past_its_short_segment_…`, when the engine renders it, then the
  layer carries the warning, once. The test lives in `crates/engine` and cites the ADR.
- [ ] Given `a_long_segment_after_a_short_one_at_a_gentle_angle_is_drawn`, when it runs, then it
  still counts 4207 pixels, and its layer carries no warning.
- [ ] Given `brush.rs`, when it is read, then `within_i32` and its call are gone, or a reason why
  the probe cannot take the place's bound stands in the hand-off.

## Read first

- Touches: `crates/render-agg/src/brush.rs`, `crates/render-agg/src/brush/tests.rs`,
  `crates/render-agg/src/lib.rs`, `crates/render/src/lib.rs` (the renderer trait),
  `crates/engine/src/layer.rs` and `crates/engine/src/lib.rs` (the layer's diagnostics), a test
  under `crates/engine/tests/`
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0025, ADR-0046; the merge message
  2156170e; `docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`.

## Blocked by

- docs/plan/revision-2026-11-18/01-q3-guarded-inputs-and-their-tests.md (Q-3's wording)

## Mode

with the human: it implements ADR-draft-third-party-failures-guarded, which waits for the
maintainer's decision, and no blocker under `docs/plan/` can name an ADR. It changes the renderer
trait and the diagnostics a map reports. Stop after the channel has landed if the probe's merge
does not fit the same session; hand off with the channel landed.
