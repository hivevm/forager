# Ticket: A multipoint of LABELCACHE OFF draws marker and label point by point

## What to build

A point layer of `LABELCACHE OFF` draws each point of a multipoint feature as
`pointLayerDrawShape` does at the pin (`src/mapdraw.c:2062-2105`): the point's markers, then its
labels, then the next point's markers. Today the engine draws all of a feature's markers and then
its labels at each point, so a later point's marker does not cover an earlier point's label;
`docs/semantics/label.md` and `docs/ARCHITECTURE.md` state that divergence, which this ticket
removes.

## Acceptance criteria

- [ ] Given a `LABELCACHE OFF` point layer with a multipoint of two points 5 px apart, a marker of
  20 px and a label at each, when it renders, then the second point's marker is drawn after, and
  over, the first point's label, with the operations in that order stated in the test.
- [ ] Given a class style with a `GEOMTRANSFORM` on such a layer, when it renders, then it draws as
  it does today.
- [ ] Given the harness, then every case holds its listed outcome and mark, and the divergence is
  gone from `label.md` and `ARCHITECTURE.md`.

## Read first

- `pointLayerDrawShape`, `src/mapdraw.c:2034-2125`, at the pin.
- `crates/engine/src/lib.rs` (the per-feature drawing of a layer of `LABELCACHE OFF`) and
  `draw_at_once` in `crates/engine/src/labels.rs`; `docs/semantics/label.md` (LABELCACHE).
- The review of #163: interleaving per vertex was judged larger than that ticket, because it
  splits a feature's drawing per point.

## Blocked by

- None (can start immediately)

## Mode

unattended: it touches nothing ADR-0007 keeps with the human (no resource limit, port, public
interface or style document format), its branch counts against ADR-0007's cap of three, and it
changes only `crates/engine` and the two documents. Stop: after two attempts that move a
reference image or a case's outcome other than through this order, or past three hours, leave
the state here and hand off.
