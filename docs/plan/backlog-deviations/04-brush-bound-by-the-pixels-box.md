# Ticket: A brushed line draws its piece at a sharp turn

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

`joins_within_i32` bounds each product of agg-rust's `DistanceInterpolator4` by adding the
pixel's offset to the join's distance as scalars. That is loose where the two point the same way:
of 1500 random roads a class, it rejects 6 paths with turns up to 90°, 29 up to 120°, 48 up to
150°, and 63 up to 170° that agg-rust draws without an overflow. The rejected piece is not drawn.

The session replaces the bound by the bound by the box the pixels lie in, which the review of
revision-2026-11-16 ticket 21 measured: the largest distance of a pixel to the join on each axis
over the segment's range grown by the bound's growth, with the difference bounded by
`|(P − X) × v|` at the four corners of that box. The bound guards undefined behaviour of AGG's C++,
so the adversarial probe runs again on it. The change deletes
`docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`.

## Acceptance criteria

- [ ] Given the path of the backlog entry, a segment of 696 pixels turning about 160° into one of
  2042 with a tile of 64 × 40, when it is brushed, then its piece is drawn.
- [ ] Given the adversarial probe of 1500 random roads a class on images of 4000 pixels, when it
  runs against the new bound, then no accepted path overflows agg-rust's interpolator, and the
  losses per class are at most those the review measured (0, 9, 13, 24).
- [ ] Given a path the old bound rejected because it overflows, when it is brushed, then it is
  still rejected and nothing panics.

## Read first

- Touches: `crates/render-agg/src/brush.rs` (`joins_within_i32`, `brushed_line`), its tests
- `docs/backlog/brushed-line-join-bound-loose-at-sharp-turns.md`; ticket 21 of
  revision-2026-11-16 and its reviews in the history, for the probe.
- ADR-0046 and ADR-0049, which bind the brush and its guard.

## Blocked by

- None (can start immediately)

## Mode

unattended: the probe and the unit tests decide the criteria, a wrong result costs one revert, and
the guard keeps its clauses of ADR-0049. Stop when the probe finds an accepted path that
overflows and no proof of the box bound covers it; hand off with the path.
