# Ticket: Hold a label's FORCE once

## What to build

From `docs/backlog/label-force-held-twice.md`. The label cache's `Member` carries `force`, read only
for a member without text (`crates/label/src/cache.rs`), and its `PointLabel` carries `force` too,
which `place` and `Cache::place` read. The engine sets both from one value at each of its two sites
(`crates/engine/src/labels.rs`). Move `force` onto `Member` alone, `place` taking it as a parameter.
`crates/label/tests/cache_pairwise.rs` builds and reads both, and its generator draws a chance for
each field: keep the draw of the dropped field, unused, so that its cases stay as they are. The
fixtures of `cache.rs`, `place.rs`, and `cache_bounds.rs` set the one field. No behaviour changes.

## Acceptance criteria

- [ ] Given `rg -n 'force' crates/label/src/place.rs`, when the change lands, then `PointLabel`
  holds no `force` field.
- [ ] Given the suite at the base, when FORCE is held once, then it passes with every expected value
  unchanged, `cache_pairwise.rs`'s generated cases among them.

## Read first

- Touches: `crates/label/src/cache.rs`, `crates/label/src/place.rs`, `crates/label/tests/`,
  `crates/engine/src/labels.rs`
- `docs/adr/0032-label-cache.md`

## Blocked by

- docs/plan/revision-2026-11-13/08-label-draw-narrowed.md

## Mode

unattended: the suite's expected values decide it; one revert. Stop when a test's expected value
moves.
