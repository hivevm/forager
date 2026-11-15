# GEOS's coordinates in the geometry module's tests are taken by hand, not recorded by the noise run

**Observed:** fidelity ticket 11 (2026-11-15). The unit tests of `crates/symbolize/src/geometry.rs`
hold the coordinates GEOS 3.12.1 gives for `simplify`, the version of the runner's `libgeos-dev`
that `scripts/build-mapserver.sh` builds MapServer against. They were taken by hand from a local
build of GEOS's release tarball, by a C program outside the tree that calls `GEOSSimplify_r` on
the coordinates `msGEOSShape2Geometry` builds. Nothing holds them to the GEOS the noise run uses:
a move of the runner's image to another GEOS would go unnoticed, and the program that took them
is not kept. ADR-draft-geometry-through-geo lists this as a follow-up.

**Where:** `crates/symbolize/src/geometry/tests.rs`; `.github/workflows/noise.yml` and the
harness's noise module `crates/cli/tests/conformance/noise/`.

**What a change weighs:**

- **A record made in the noise run, as ADR-0038 records PROJ's:** a small C program beside
  `crates/cli/tests/conformance/mapserver_proj.c`, built against the runner's GEOS, writes the
  results of a list of inputs to a record in the tree; the noise run holds the record against the
  program, and the unit tests read the record. It needs an ADR of its own or a successor of
  ADR-0038, and a run on the runner to fill the record.
- **Keeping the hand-taken coordinates:** no work, at the cost of the drift named above.
- `buffer` (fidelity ticket 12) adds GEOS's coordinates of its own, which the same record would
  hold.
