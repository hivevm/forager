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
The order of a multipoint feature's markers and labels, which reworks the same loop, is taken
over below; it lands first, as a commit of its own.

## Acceptance criteria

- [ ] Given the tests of `LABELCACHE OFF` in `crates/engine/tests/labels.rs`, when they run, then
  they pass unchanged.
- [ ] Given a layer of `LABELCACHE OFF` with 20,000 points, when it renders in release, then the
  time the pull request states is not above today's, measured the same way.
- [ ] Given the code, when it is read, then `collect` takes no flag of the layer's mode, and no
  `Cached` or `Def` is built for a label drawn at once.

## Taken over from `docs/plan/labelcache-off-multipoint-order/01-labelcache-off-multipoint-order.md`

Both rework the layer's loop in `crates/engine/src/lib.rs`; one change carries both, the behaviour
of the order a commit typed `fix` of its own beside the refactor (docs/CONVENTIONS.md), on the
maintainer's word (triage of 2026-11-07).

A point layer of `LABELCACHE OFF` draws each point of a multipoint feature as
`pointLayerDrawShape` does at the pin (`src/mapdraw.c:2062-2105`): at each point in the extent,
every class style's marker by `msDrawMarkerSymbol` (2072-2085), then each label `msGetLabelStatus`
passes by `msDrawLabel` (2086-2105), then the next point's markers. Today the engine draws all of a
feature's markers and then its labels at each point (the layer's loop in
`crates/engine/src/lib.rs`), so a later point's marker does not cover an earlier point's label;
`docs/semantics/label.md` (LABELCACHE) and `docs/ARCHITECTURE.md` state that divergence, which
this ticket removes.

A class style's `GEOMTRANSFORM` does not change this: `pointLayerDrawShape` calls
`msDrawMarkerSymbol` for every style at every point, whatever its transform (2072-2085), and the
engine's point branch does the same (`draw_ops` in `crates/symbolize/src/draw.rs`). The
interleaving keeps that.

Its criteria, now this ticket's as well:

- [ ] Given a `LABELCACHE OFF` point layer with a multipoint of two points 5 px apart, a marker of
  20 px and a label at each, when it renders, then the operations are the first point's marker, the
  first point's label, the second point's marker, the second point's label, in that order, stated
  in the test with `src/mapdraw.c:2062-2105` cited.
- [ ] Given the same layer whose class style has `GEOMTRANSFORM "start"`, when it renders, then a
  marker of that style is drawn at each of the two points, each before that point's label, as
  `msDrawMarkerSymbol` at `src/mapdraw.c:2072-2085` draws it regardless of the transform.
- [ ] Given a point of the multipoint outside the map's extent, when it renders, then neither its
  marker nor its label is drawn, as the `msPointInRect` test at `src/mapdraw.c:2065-2066` skips the
  point before both.
- [ ] Given the harness, when it runs, then every case holds its listed outcome and `labels-hold`
  mark, and the divergence is gone from `label.md` and `ARCHITECTURE.md`.

## Read first

- [ADR-0031](../../adr/0031-label-glyphs.md),
  [ADR-0032](../../adr/0032-label-cache.md); `crates/engine/src/labels.rs`
  (`collect`, `feature`, `draw_at_once`, `cache`), `crates/engine/src/lib.rs` (the layer's loop);
  `pointLayerDrawShape` and `msDrawLabel` in `src/mapdraw.c` at the pin;
  [label.md](../../semantics/label.md).

## Blocked by

- None (can start immediately)

## Mode

unattended: the existing tests decide behaviour, and the timing the pull request states decides
the cost; its scope is two files of `engine`, and a wrong result costs one revert; it touches none
of the parts [ADR-0006](../../adr/0006-harness.md) keeps with the maintainer and counts against its
cap of three; a wrong result shows as a label test or the conformance list's `labels-hold`
moving; stop: when a test of `LABELCACHE OFF` would have to change, the session records why here
and hands off.
