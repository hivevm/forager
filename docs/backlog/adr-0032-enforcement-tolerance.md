# ADR-0032's Enforcement holds an offset of whole cells and one of 3,000 px to one bound

Observed: ADR-0032's Enforcement has labels drawn beyond the image ask for no more boxes than the
same labels unmoved within the image. That holds exactly for an offset that is a whole number of
cells of the finest level (1,000,000 px), and only by the cell alignment of the cases for others:
an offset of 3,000 px, or one within the image, moves the count by up to ±10 % either way. With
6.46 px between points, the unmoved labels ask for 12.252 boxes each, over the 12 the Enforcement
names for the upper left. The test `off_image_labels_cost_what_the_same_labels_cost_within_it` in
`crates/label/tests/cache_bounds.rs` holds what is true today: equal counts for whole cells, no
more for 3,000 px at the two spacings it tries. Review of ticket revision-2026-11-13/25.

The maintainer chose on 2026-11-14 to word the Enforcement at the next normalization, as "whole-cell
offsets exactly, others within a tolerance", rather than supersede ADR-0032 now.

Where: docs/adr/0032-label-cache.md (Enforcement), crates/label/tests/cache_bounds.rs.
