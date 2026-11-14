# ADR-0032: The label cache tests collisions over a grid that wraps around the image, with MapServer's outcome, and holds at most 100,000 labels a render

- **Status:** 🟢 accepted
- **Date:** 2026-11-13
- **Deciders:** Markus Brigl
- **Applies to:** the label cache of `crates/label` and the engine's use of it in `crates/engine`: how a label is tested against the markers and the labels drawn before it, the grid that bounds those tests, and how many labels one render holds

## Context

MapServer's `msTestLabelCacheCollisions` (`src/maplabel.c` at the pin) tests a label against
every marker of its priority or above and every label drawn before it, one by one. Its cost is the
square of the labels in a render, and the cache spans every layer of the map. Q-9 asks that no
input exhausts time without bound, and bounds the labels one render's label cache holds at
100,000. It also bounds the memory of a label the cache holds at 400 bytes, measured by a test.
G-10 asks that labels are placed and discarded as MapServer places and discards them.

A port that tests pair by pair is MapServer's arithmetic, and quadratic: measured on 2026-10-30,
labels that never meet but stand at one point cost some 34 ns a pair, so 16,000 of them take
8.6 s, and a million, which Q-9's bound on the features of one layer still lets through, some
nine hours a layer. A grid that visits only the boxes near a label keeps a typical render near
linear, but labels piled at one point still meet the same cell, and a grid that copies a box into
every cell it covers multiplies time and memory by the cells (4000 labels of a negative `BUFFER`
took 119 s and 32 KB each in a first version).

The decision of 2026-10-30 chose such a grid, of levels over the image, each box held once. Its code
held every coordinate past the image at the image's edge. That decision chose a box's level by "the
box clamped to the image" and was silent on whether the cells cover the image alone. A label that
may be partial is drawn wherever it lands, however far outside: a large `OFFSET`, or a point outside
the extent's image, moves it there. Such labels were filed in the cells along the edge, and a test
near the edge visited all of them (the backlog entry `label-cache-edge-cells`, taken up as ticket
`revision-2026-11-13/25`). Measured during the revision of 2026-10-31: 100,000 labels offset 3,000
px right and down on a 4,096 px image, `PARTIALS TRUE`, ask for about 5,000 boxes per label with
their points spread over the whole image; the same measure for a spread over the image's upper left,
recorded then as about 10,000, did not reproduce in 2026-11-13's prototype. Labels that stay within
the image ask for 11 to 12.2, by their density. The cost stays bounded by the 100,000 labels of Q-9,
and the outcome is MapServer's; only the time per label is far from that of labels in the image.

The maintainer chose on 2026-11-12 to leave the grid as it was. On 2026-11-13 the maintainer chose
that the grid covers labels drawn beyond the image. The criteria come from the ticket of that
revision:

- the off-image case above asking for no more boxes per label than the same labels within the
  image, as the maintainer read the ticket's bound on 2026-11-13: dense labels within the image
  ask for about 12.2 boxes each, so the bound is the density's, not the edge's; and fewer than 12
  where their points spread over the image's upper left;
- the memory within that test's bound per label, under 64 bytes;
- `crates/label/tests/cache_pairwise.rs` passing unchanged in its assertion.

Two forces narrow the shape. A label's box is known only once it is placed, so no grid can be
sized to the extent of the boxes before the walk starts. Coordinates past the image are not
bounded at all: offsets, points, and buffers reach any finite value, and infinity. Any structure
whose cells follow the coordinates either grows with them, against Q-9's memory, or clamps them
somewhere, and the pile moves to that edge.

The gutter of `labelcache_map_edge_buffer` was implemented in commit `addb908`, with the grid
unchanged. `msDrawLabelCache` reads it from the map's `WEB` `METADATA`. `labelInImage` then
requires a label that may not be partial to lie that many pixels inside each edge. The decision of
2026-10-30 named the gutter out of scope; this record states where it stands.

## Decision

We will test a label's collisions over a loose grid that wraps around the image and visits only the
boxes that can meet it. The grid must hold each marker and each drawn label once. Every collision
must be decided as `msTestLabelCacheCollisions` decides it. The cache of one render must hold at
most 100,000 labels and discard those past it without a test and with one warning.

- **Grid.**
  - Level `k` has square cells of side `64 · 2^k`, up to the first level whose side covers the
    image's width and height.
  - Each level has the columns and rows that cover the image, its far edge included. The cells
    must not depend on where the boxes lie.
  - A box is held once, in the finest level whose side its own extent does not exceed, at the
    cell of its lower corner.
  - A box larger than the coarsest level's side is held in the coarsest level.
  - The grid must not clamp a coordinate to the image. A coordinate's cell index along the plane
    is taken modulo the level's columns, or its rows, so the grid wraps around.
  - A cell index beyond the range of a 64-bit integer saturates at that range's end.
  - A test visits the cells from one before its own to that of its far edge, wrapped alike, in
    each level that holds anything.
  - A test must visit each cell at most once. Where its run of cells is as long as the level's
    columns or rows, it visits every column or row once.
  - A box with an axis inside out is held at its smaller edge on that axis. It is never tested
    against another box inside out on the same axis, since such boxes cannot meet.
  - A box that is not a number is visited by every test, as MapServer's comparisons on it are
    true.
- **Gutter.** The gutter of `labelcache_map_edge_buffer` narrows only `labelInImage`'s test of a
  label that may not be partial. The grid must ignore the gutter: its cells and its wrap are those
  of the whole image.
- **Outcome.** Which labels are drawn and which discarded must be the outcome of MapServer's
  pairwise test on the same labels, in its order, with the same gutter. The grid only spares the
  pairs that cannot meet.
- **Bound.** The 100,001st label of a render, in the order the cache takes them, and every label
  after it are discarded. The render reports `labels-over-limit` once (Q-9).

**Out of scope:** the label cache of line and polygon labels; `LABELCACHE OFF`, which draws a
layer's labels outside the cache.

## Alternatives considered

- **Pairwise, as MapServer** — the plainest port, and quadratic in the labels of a render: hours
  for inputs Q-9 lets through.
- **A grid without a bound** — near linear for typical maps, but labels piled at one point stay
  quadratic, against Q-9.
- **A tighter bound of 10,000 labels** — a worst case of seconds, but maps with many small labels
  lose labels MapServer draws.
- **An R-tree** — the same visits for boxes of every size, but a dependency or more code than
  the levels of a grid, for no gain the measurements show.
- **The grid over the image, coordinates held at its edge** — what the decision of 2026-10-30
  chose: a box held once, in the finest level whose cells are no smaller than the box clamped to
  the image, and the gutter of `labelcache_map_edge_buffer` out of scope. Kept on 2026-11-12 as
  the smallest variant. Labels drawn beyond the image pile into the edge cells, some 5,000 to
  10,000 boxes per label in the measured case against fewer than 12 within the image.
- **Cells past the image by a bounded margin, a catch-all beyond** — a margin of one image on each
  side multiplies the cells, and the grid's fixed memory, by nine. It breaks the test's 64 bytes
  per label for 4000 labels. An offset past the margin piles the labels into the catch-all, which
  every test visits, so the measurement returns with a larger `OFFSET`.
- **Clamping per level, with a separate structure for boxes outside the image** — the separate
  structure needs its own bounded index over unbounded coordinates, which is the problem again. A
  box across the image's edge must then be held or looked up in both.
- **A grid over the extent of all boxes** — a label's box is known only after its placement, so
  the extent needs a pass over every candidate position first. A fixed count of cells over a wide
  extent makes them coarse, and one far label piles every label in the image into a few cells. A
  fixed cell side over a wide extent grows the cells, and the memory, without bound.
- **A sparse grid, cells in a hash map keyed by their unbounded index** — memory per entry, with
  no aliasing between far boxes and near ones. A query box wide at a fine level spans a number of
  cells without bound, so its run must be capped by a scan of the level's occupied cells. Hash
  collisions visit an entry twice unless deduplicated. More code than a modulo, for a gain no
  measurement asks for.
- **A wider torus, of two images along each axis** — fewer far boxes share a cell with near ones,
  for four times the grid's fixed memory. The measured case needs no more than one image.

## Sources / Prior art

- MapServer `rel-8-6-6`: `msDrawLabelCache` in `src/mapdraw.c`; `msTestLabelCacheCollisions`,
  `labelInImage`, `intersectLabelPolygons`, and `msRectOverlap` in `src/maplabel.c` and
  `src/mapsearch.c`.
- Loose grids and quadtrees for collision broad phases, as described in Ericson, *Real-Time
  Collision Detection* (Morgan Kaufmann, 2005), chapter 7 *Spatial Partitioning*. Its section
  7.1.3 *Hashed Storage and Infinite Grids*, under 7.1 *Uniform Grids*, maps an unbounded grid
  onto a finite array, of which the wrap is the modulo form; checked 2026-11-14 against the
  book's table of contents at realtimecollisiondetection.net/books/rtcd/toc.
- Teschner, Heidelberger, Müller, Pomeranets, and Gross, *Optimized Spatial Hashing for Collision
  Detection of Deformable Objects* (VMV 2003, Munich), pages 47–54: a hash function maps a
  potentially infinite regular grid onto a table of fixed size; checked 2026-11-14.
- The measurements above: those of the decision of 2026-10-30, from the work and the review of
  the labels spec, and the edge cells', from the ticket
  `revision-2026-10-31/11-label-cache-files-drawn-boxes` (#180) and its backlog entry, taken up
  as ticket `revision-2026-11-13/25`.
- Commit `addb908`, the gutter of `labelcache_map_edge_buffer`.

## Consequences

- Positive: a render's labels cost near linear time for typical maps and a bounded time for any
  input; the collisions are MapServer's.
- Positive: labels beyond the image spread over the cells as labels within it do. In the measured
  case their coordinates run from 3,000 to some 7,100 px, less than the finest level's span of
  4,160 px, so no two of them share a cell they would not share in the plane. This is expected
  from the construction, not yet measured.
- Positive: the grid's cells stay those of the earlier grid's code. Labels that piled into one
  clamped cell now spread over many, each occupied cell with a buffer of its own, which a cell's
  first entry allocates exactly. Q-9's 400 bytes per label and the test's 64 hold.
- Positive: the gutter is placed: a test of the image, not of the grid.
- Negative / trade-offs: the grid is code MapServer does not have, which a reader of the port
  must not take for a difference in outcome; a map of more than 100,000 labels loses labels
  MapServer would draw, with a warning.
- Negative / trade-offs: a box a whole span of a level away from another shares its cell. A test
  then visits it and finds by `msRectOverlap` that it does not meet. Labels laid out at exactly
  that span, which no map is expected to give, pile as labels at one point pile. The bound of
  100,000 labels still caps them.
- Negative / trade-offs: a box across the image's edge and larger than the coarsest level's side
  is held in that level by its own extent, where the earlier grid's code held it finer by its part
  in the image. Every test visits the coarsest level whole, as before for boxes larger than the
  image.
- Follow-ups: whether the bound moves once the cost of a label test is measured again; the label
  cache of line labels; whether a wider torus pays, should a measured map alias far labels onto
  near ones.

## Enforcement

Tests cite this ADR with `Verifies:`:

- in `crates/label/tests/cache.rs` and `crates/label/tests/cache_bounds.rs`, a collision that the
  grid must find across cells (a box that is not a number, a box larger than its cell), and the
  bounded visits and memory of piled, large, and inside-out boxes;
- in `crates/label/tests/cache_bounds.rs`, `MAX_LABELS` labels drawn beyond the image ask for as
  many boxes as the same labels unmoved within the image, exactly for an offset of whole cells of
  the finest level and within a tolerance for others. An offset of 1,000,000 px, a whole number of
  such cells, asks for exactly as many. An offset of 3,000 px, as one within the image, moves the
  count by up to ±10 % either way with the spacing of the points; at the two spacings the test
  tries, it asks for no more. Where their points stand 3.4 px apart over the image's upper left,
  they ask for fewer than 12 boxes per label. They allocate under 64 bytes per label at the peak.
  The labels are offset right and down, and left and up, with `PARTIALS TRUE`, from points spread
  over the whole image and over its upper left. Those of the upper left offset by 3,000 px stay
  within the image;
- in `crates/label/tests/cache.rs`, two labels a level's span apart, and labels at an infinite
  coordinate, meet as `msRectOverlap` decides;
- in `crates/label/tests/cache_pairwise.rs`, the grid's outcome against MapServer's pairwise test
  on labels of arbitrary boxes, with and without a gutter. Some of their points lie several
  spans of the image away from it, on either side;
- in `crates/engine/tests/labels.rs`, the 100,001st label discarded with `labels-over-limit`.
