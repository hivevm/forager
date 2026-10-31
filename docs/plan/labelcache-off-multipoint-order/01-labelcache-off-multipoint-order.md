# Ticket: A multipoint of LABELCACHE OFF draws marker and label point by point

## What to build

A point layer of `LABELCACHE OFF` draws each point of a multipoint feature as
`pointLayerDrawShape` does at the pin (`src/mapdraw.c:2062-2105`): at each point in the extent,
every class style's marker by `msDrawMarkerSymbol` (2072-2085), then each label `msGetLabelStatus`
passes by `msDrawLabel` (2086-2105), then the next point's markers. Today the engine draws all of a
feature's markers and then its labels at each point (`crates/engine/src/lib.rs:176-190`), so a
later point's marker does not cover an earlier point's label; `docs/semantics/label.md`
(LABELCACHE) and `docs/ARCHITECTURE.md` state that divergence, which this ticket removes.

A class style's `GEOMTRANSFORM` does not change this: `pointLayerDrawShape` calls
`msDrawMarkerSymbol` for every style at every point, whatever its transform (2072-2085), and the
engine's point branch does the same (`crates/engine/src/draw.rs:261-266`). The interleaving keeps
that.

## Acceptance criteria

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

- `pointLayerDrawShape`, `src/mapdraw.c:2034-2125`, at the pin.
- `crates/engine/src/lib.rs` (the per-feature drawing of a layer of `LABELCACHE OFF`, 168-190),
  `draw_at_once` in `crates/engine/src/labels.rs`, and the point branch of `draw_ops` in
  `crates/engine/src/draw.rs`; `docs/semantics/label.md` (LABELCACHE).
- The review of #163: interleaving per vertex was judged larger than that ticket, because it
  splits a feature's drawing per point.

## Blocked by

- None (can start immediately)

## Mode

unattended: engine tests whose order and values come from `pointLayerDrawShape` decide each
criterion; the change stays in `crates/engine` and the two documents, undone by one revert. It
touches nothing [ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, port,
public interface or style document format, only the order in which the engine draws what it already
draws. Its branch counts against ADR-0007's cap of three open agent branches. A wrong result shows
as an operation order a test pins that moves, or a case outcome or `labels-hold` mark that moves,
and is undone by the revert; the tests citing the C prove it right. Stop: if any case's listed
outcome or `labels-hold` mark moves, if a reference image of Forager's moves whose map has no
multipoint feature on a layer of `LABELCACHE OFF`, or if the order needs a change outside
`crates/engine`, leave the state here and hand off.
