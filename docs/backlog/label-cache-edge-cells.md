# Labels drawn beyond the image pile into the label cache's edge cells

Observed while working `revision-2026-10-31/11-label-cache-files-drawn-boxes` (#180): the grid of
the label cache (`Grid` in `crates/label/src/cache.rs`, `Grid::place`) holds a box's coordinates
past the image at the image's edge. ADR-0032 chooses a box's level by "the box clamped to the image"
and holds it "at the cell of its lower corner"; that the cells cover the image alone is the code's.
A label that may be partial is drawn wherever it lands, however far outside, so labels moved off the
image, by a large `OFFSET` or points outside the extent's image, are filed in the cells along the
edge, and a test near the edge visits all of them. Measured: 100,000 labels offset 3,000 px right
and down on a 4,096 px image, `PARTIALS TRUE`, ask for thousands of boxes per label, about 10,000
with the points spread over the image's upper left and about 5,000 spread over all of it, against
fewer than 12 for labels that stay within the image. The cost stays bounded by `MAX_LABELS` (Q-9),
and the outcome is MapServer's.

The maintainer chose on 2026-11-12 the smallest variant, which changes no ADR: the grid stays as it
is, and this waits. The ticket read cells past the image as a change of ADR-0032's grid; a review
read the Decision as silent on whether the cells cover only the image, so they might come within it.
Whoever takes this up decides that reading first, by a supersession of ADR-0032 or within it, with
the criteria the ticket gave: the case above within the bound of
`crates/label/tests/cache_bounds.rs`'s case of labels offset within the image (fewer than 12 boxes
per label), the memory within that test's bound per label, and
`crates/label/tests/cache_pairwise.rs` unchanged.

Where: crates/label/src/cache.rs (`Grid`), crates/label/tests/cache_bounds.rs,
crates/label/tests/cache_pairwise.rs; docs/adr/0032-label-cache.md.
