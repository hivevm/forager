# Clauses of IR-c-ports-in-style-ir, GEO-own-circle-for-buffer, REN-i-overlay-shared-with-geo, GEO-simplifypt-ported-from-jts, IR-geomtransform-with-simplifypt, and ARCH-third-party-failures-guarded that are spent or stale

**Observed:** the design revision of 2026-11-16, and ticket revision-2026-11-16/11. Clauses of
accepted ADRs that bound only the order of the fidelity plan's work, state one rule twice, or list
what has since grown. Three of them came from the fidelity plan:

- GEO-own-circle-for-buffer *Precondition*: it must not be accepted before
  REN-i-overlay-shared-with-geo. Both are accepted.
- IR-geomtransform-with-simplifypt *Order of landing*: each of `simplify`, `buffer`, and
  `simplifypt` enters the tree in the change that draws it. All three are drawn, and none is a
  `not_drawn` reason any more.
- GEO-own-circle-for-buffer *Dependency* and REN-i-overlay-shared-with-geo *The dependency* both
  state that `symbolize` alone among the workspace's crates depends on geo. A change to the rule has
  two places to change.

GEO-own-circle-for-buffer restates the clauses of GEO-simplify-buffer-through-geo whole (ticket
backlog-deviations/05, accepted 2026-11-19), so its *Precondition* and *Dependency* above stand as
they stood before. The restatement keeps three more stale passages, as the maintainer accepted its
text:

- its *Context* names the clause that i_overlay be a dependency of `render-agg` alone as a force
  of today, of REN-hatch-cut-by-i-overlay (superseded by REN-i-overlay-shared-with-geo);
- its *Context* cites "its table" of a superseded ADR, the first of the chain
  IR-typed-geomtransform, IR-geomtransform-with-simplify-buffer, IR-geomtransform-with-simplifypt;
- its paragraph *The point's circle* says GEOS 3.12.1's factor of a dropped vertex is not confirmed.
  The review of ticket backlog-deviations/05 read it in GEOS 3.12.1's
  `OffsetSegmentGenerator.cpp:51`: 1.0E-6, as the ADR decides.

IR-c-ports-in-style-ir's *The helpers* lists the ports of `crates/style-ir/src/c.rs` by name, but
not all of them: `ms_compute_bounds`, `ms_get_symbol_index`, and the opacity's alpha (7d09373f,
noted by the revision of 2026-11-15), and `point_in_polygon` (ticket revision-2026-11-16/11). Its
general clause, one port in `c` that every crate calls, covers them; the list is the stale part.

GEO-own-circle-for-buffer, GEO-simplifypt-ported-from-jts, and IR-geomtransform-with-simplifypt
decide one thing between them: the transform tree's nodes and the geometry code that computes them.

ARCH-third-party-failures-guarded's *Enforcement* asks a test in `crates/engine` that the layer
warns once for points of a brushed line that are no finite number. By the maintainer's ruling of
2026-11-18 (ticket revision-2026-11-18/03, merged in f4e57a15), leaving such a point out is a rule
of the display list, not a guard, and it warns nothing (ticket revision-2026-11-18/11). The item is
spent. Its follow-up "Q-3's wording" is settled too: ticket revision-2026-11-18/01 (merge 0f9b133b)
gave Q-3 a guard's undrawn input as a diagnostic, and the ADR's accepted text still lists it as open
(the design revision of 2026-11-18 (2)).

**Where:** `docs/adr/IR-c-ports-in-style-ir.md`, `docs/adr/GEO-own-circle-for-buffer.md`,
`docs/adr/REN-i-overlay-shared-with-geo.md`, `docs/adr/GEO-simplifypt-ported-from-jts.md`,
`docs/adr/IR-geomtransform-with-simplifypt.md`, `docs/adr/ARCH-third-party-failures-guarded.md`.

**What a change weighs:**

- **Restated at the next normalization**, which only the maintainer starts: the spent clauses
  dropped, the geo rule stated once, and GEO-own-circle-for-buffer, GEO-simplifypt-ported-from-jts,
  and IR-geomtransform-with-simplifypt consolidated or kept apart, as the maintainer decides.
- **Left as they are**: no work. The spent clauses bind nothing, and the duplicated rule holds
  while both ADRs stand.
