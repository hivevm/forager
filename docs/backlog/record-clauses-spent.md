# Clauses of ADR-0017, ADR-0041, ADR-0042, ADR-0047, ADR-0048, and ADR-0049 that are spent or stale

**Observed:** the design revision of 2026-11-16, and ticket revision-2026-11-16/11. Clauses of
accepted ADRs that bound only the order of the fidelity plan's work, state one rule twice, or list
what has since grown. Three of them came from the fidelity plan:

- ADR-0041 *Precondition*: it must not be accepted before ADR-0042. Both are accepted.
- ADR-0048 *Order of landing*: each of `simplify`, `buffer`, and `simplifypt` enters the tree in
  the change that draws it. All three are drawn, and none is a `not_drawn` reason any more.
- ADR-0041 *Dependency* and ADR-0042 *The dependency* both state that `symbolize` alone among the
  workspace's crates depends on geo. A change to the rule has two places to change.

ADR-0017's *The helpers* lists the ports of `crates/style-ir/src/c.rs` by name, but not all of
them: `ms_compute_bounds`, `ms_get_symbol_index`, and the opacity's alpha (7d09373f, noted by the
revision of 2026-11-15), and `point_in_polygon` (ticket revision-2026-11-16/11). Its general
clause, one port in `c` that every crate calls, covers them; the list is the stale part.

ADR-0041, ADR-0047, and ADR-0048 decide one thing between them: the transform tree's nodes and the
geometry code that computes them.

ADR-0049's *Enforcement* asks a test in `crates/engine` that the layer warns once for points of a
brushed line that are no finite number. By the maintainer's ruling of 2026-11-18 (ticket
revision-2026-11-18/03, merged in f4e57a15), leaving such a point out is a rule of the display list,
not a guard, and it warns nothing (ticket revision-2026-11-18/11). The item is spent. Its follow-up
"Q-3's wording" is settled too: ticket revision-2026-11-18/01 (merge 0f9b133b) gave Q-3 a guard's
undrawn input as a diagnostic, and the ADR's accepted text still lists it as open (the design
revision of 2026-11-18 (2)).

**Where:** `docs/adr/0017-c-ports-in-style-ir.md`, `docs/adr/0041-geometry-through-geo.md`,
`docs/adr/0042-i-overlay-in-render-agg-and-geo.md`, `docs/adr/0047-simplifypt-ported-from-jts.md`,
`docs/adr/0048-transform-tree-with-simplifypt.md`, `docs/adr/0049-third-party-failures-guarded.md`.

**What a change weighs:**

- **Restated at the next normalization**, which only the maintainer starts: the spent clauses
  dropped, the geo rule stated once, and ADR-0041, ADR-0047, and ADR-0048 consolidated or kept
  apart, as the maintainer decides.
- **Left as they are**: no work. The spent clauses bind nothing, and the duplicated rule holds
  while both ADRs stand.
