# Ticket: Settle a layer's labels once, and draw a `LABELCACHE OFF` label without the cache's records

## What to build

A point layer of `LABELCACHE OFF` (42bed52) draws its labels through the label cache's structures
and a widened entry point:

- `Labels::collect` took a flag `point_layer` and an `Option` of the renderer `at_once`
  (`crates/engine/src/labels.rs`), and `feature` has seven arguments under
  `allow(clippy::too_many_arguments)`. `lib.rs` calls `collect` once per feature through
  `chunks(1)`, with `point_layer` written `true`, so what holds for the whole layer, its fixed
  labels, the set of labels with a text, and `Drawing::draw`'s plans, is computed again per
  feature.
- For a label drawn at once, `feature` still builds a `Cached` whose path and text index entries
  never pushed, registers a `Def` no member uses, and computes a `labelpnt` marker it throws away;
  `draw_at_once` builds a `LabelStyles` and a `Styled` with a fresh `Canvas` per feature only to
  rebuild styles already bound; and the walk of a feature's vertices in the extent is written in
  both `cache` and `draw_at_once`.

Settle a layer's labels once, before its features; label each feature through one function that
the cache and the at-once path both call; and draw a label at once from its bound values, without
the cache's records.
`docs/plan/labelcache-off-multipoint-order/01-labelcache-off-multipoint-order.md` reworks the
same loop for its order: land that one first, or fold this into it with the maintainer's word.

## Acceptance criteria

- [ ] Given the tests of `LABELCACHE OFF` in `crates/engine/tests/labels.rs`, when they run, then
  they pass unchanged.
- [ ] Given a layer of `LABELCACHE OFF` with 20,000 points, when it renders in release, then the
  time the pull request states is not above today's, measured the same way.
- [ ] Given the code, when it is read, then `collect` takes no flag of the layer's mode, and no
  `Cached` or `Def` is built for a label drawn at once.

## Read first

- [ADR-0028](../../adr/0028-text-stack.md),
  [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md); `crates/engine/src/labels.rs`
  (`collect`, `feature`, `draw_at_once`, `cache`), `crates/engine/src/lib.rs` (the layer's loop);
  `pointLayerDrawShape` and `msDrawLabel` in `src/mapdraw.c` at the pin;
  [label.md](../../semantics/label.md).

## Blocked by

- docs/plan/labelcache-off-multipoint-order/01-labelcache-off-multipoint-order.md

## Mode

unattended: the existing tests decide behaviour, and the timing the pull request states decides
the cost; its scope is two files of `engine`, and a wrong result costs one revert; it touches none
of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its
cap of three; a wrong result shows as a label test or the conformance list's `labels-hold`
moving; stop: when a test of `LABELCACHE OFF` would have to change, the session records why here
and hands off.
