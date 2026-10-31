# ADR-0035: The label cache tests collisions over a grid, with MapServer's outcome, and holds at most 100,000 labels a render

- **Status:** 🟢 accepted
- **Date:** 2026-10-30
- **Deciders:** Markus Brigl
- **Applies to:** the label cache of `crates/label` and the engine's use of it in `crates/engine`: how a label is tested against the markers and the labels drawn before it, and how many labels one render holds

## Context

MapServer's `msTestLabelCacheCollisions` (`src/maplabel.c` at the pin) tests a label against
every marker of its priority or above and every label drawn before it, one by one. Its cost is the
square of the labels in a render, and the cache spans every layer of the map. Q-9 asks that no
input exhausts time without bound; G-10 asks that labels are placed and discarded as MapServer
places and discards them.

A port that tests pair by pair is MapServer's arithmetic, and quadratic: measured on 2026-10-30,
labels that never meet but stand at one point cost some 34 ns a pair, so 16,000 of them take
8.6 s, and a million, which Q-9's bound on the features of one layer still lets through, some
nine hours a layer. A grid that visits only the boxes near a label keeps a typical render near
linear, but labels piled at one point still meet the same cell, and a grid that copies a box into
every cell it covers multiplies time and memory by the cells (4000 labels of a negative `BUFFER`
took 119 s and 32 KB each in a first version).

## Decision

We will test a label's collisions in the label cache over a loose grid that holds each marker and
each drawn label once and visits only those that can meet it, deciding every collision as
`msTestLabelCacheCollisions` decides it, and hold at most 100,000 labels in the cache of one
render, discarding those past it without a test and with one warning.

- **Grid.** A box is held once, in the finest level whose cells are no smaller than the box
  clamped to the image, at the cell of its lower corner; a test visits the cells from one before
  its own to that of its far edge, in each level that holds anything. A box with an axis inside
  out is held at its smaller edge on that axis and never tested against another inside out on
  the same axis, since such boxes cannot meet; a box that is not a number is visited by every
  test, as MapServer's comparisons on it are true.
- **Outcome.** Which labels are drawn and which discarded is the outcome of MapServer's pairwise
  test on the same labels, in its order; the grid only spares the pairs that cannot meet.
- **Bound.** The 100,001st label of a render, in the order the cache takes them, and every label
  after it is discarded, and the render reports `labels-over-limit` once (Q-9).

**Out of scope:** the label cache of line and polygon labels; `LABELCACHE OFF`, which draws a
layer's labels outside the cache; the gutter of `labelcache_map_edge_buffer`.

## Alternatives considered

- **Pairwise, as MapServer** — the plainest port, and quadratic in the labels of a render: hours
  for inputs Q-9 lets through.
- **A grid without a bound** — near linear for typical maps, but labels piled at one point stay
  quadratic, against Q-9.
- **A tighter bound of 10,000 labels** — a worst case of seconds, but maps with many small labels
  lose labels MapServer draws.
- **An R-tree** — the same visits for boxes of every size, but a dependency or more code than
  the levels of a grid, for no gain the measurements show.

## Sources / Prior art

- MapServer `rel-8-6-6`: `msDrawLabelCache` in `src/mapdraw.c`; `msTestLabelCacheCollisions`,
  `intersectLabelPolygons`, and `msRectOverlap` in `src/maplabel.c` and `src/mapsearch.c`.
- Loose grids and quadtrees for collision broad phases, as described in Ericson, *Real-Time
  Collision Detection* (2005), chapter 7.
- The measurements above, from the work and the review of ticket labels/06.

## Consequences

- Positive: a render's labels cost near linear time for typical maps and a bounded time for any
  input; the collisions are MapServer's.
- Negative / trade-offs: the grid is code MapServer does not have, which a reader of the port
  must not take for a difference in outcome; a map of more than 100,000 labels loses labels
  MapServer would draw, with a warning.
- Follow-ups: whether the bound moves once the cost of a label test is measured again; the label
  cache of line labels.

## Enforcement

Tests of `crates/label` cite this ADR with `Verifies: ADR-0035`: a collision that the grid must
find across cells (a box that is not a number, a box larger than its cell), the bounded visits and
memory of piled, large, and inside-out boxes, and the 100,001st label discarded with
`labels-over-limit`.
