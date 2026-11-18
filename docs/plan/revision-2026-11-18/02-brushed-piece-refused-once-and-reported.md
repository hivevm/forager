# Ticket: A brushed line's piece past agg-rust's `i32` is refused by one probe and reported

## What to build

`render-agg` refuses a piece of a brushed line in two steps (`crates/render-agg/src/brush.rs`):

- `within_i32` bounds the pattern's place by the longest segment, which `filtered` ports AGG's
  vertex filter again to find;
- `JoinProbe` runs agg-rust's outline rasterizer once without pixels and bounds the joins.

The probe's `line3` already receives each kept segment's `lp.len`, the length `within_i32` takes
from `filtered`. Each piece is walked three times, and both steps refuse silently. A user sees a
line with a gap and no diagnostic.

As ADR-draft-third-party-failures-guarded decides once the maintainer has answered it:

- the probe takes the place's bound too, so one guard and one pass before the draw remain, and
  `filtered` goes if nothing else uses it;
- a refused piece is reported to the layer that draws it, with the code the maintainer chooses
  (`symbol-not-drawn` or one of its own);
- the guard carries the marker `// Guard: agg-rust 1.2.0` with the lines it was derived from.

The renderer trait of `crates/render` returns nothing from `draw` today. How a refusal in
`render-agg` reaches the layer's diagnostics is the first question of this ticket. It changes
the trait every renderer implements.

## Acceptance criteria

- [ ] Given the hairpin of `brush/tests.rs` at `HAIRPIN_RUN` and one subpixel past it, when
  each is drawn, then the first draws as today and the second draws nothing and is reported.
- [ ] Given the fuzz path of `a_piece_whose_join_lies_far_past_its_short_segment_…`, when it is
  rendered through the engine, then the layer carries the warning the maintainer chose.
- [ ] Given `a_long_segment_after_a_short_one_at_a_gentle_angle_is_drawn`, when it runs, then it
  still counts 4207 pixels.
- [ ] Given `brush.rs`, when it is read, then `within_i32` and its call are gone, or a reason why
  the probe cannot take the place's bound stands in the hand-off.

## Read first

- Touches: `crates/render-agg/src/brush.rs`, `crates/render-agg/src/brush/tests.rs`,
  `crates/render/src/lib.rs` (the renderer trait), `crates/engine/src/layer.rs` (the layer's
  diagnostics)
- `docs/adr/draft-third-party-failures-guarded.md`; ADR-0025, ADR-0046; the merge message
  2156170e; `docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`.

## Blocked by

- docs/plan/revision-2026-11-18/01-q3-guarded-inputs-and-their-tests.md (is a refusal warned)

## Mode

with the human: it waits for the maintainer's decision of
`docs/adr/draft-third-party-failures-guarded.md`, which no blocker under `docs/plan/` can name. It
implements a proposed ADR whose follow-ups are open, and it changes the
renderer trait and the diagnostics a map reports.
