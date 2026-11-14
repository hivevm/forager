# Ticket: The core's reprojection is held to PROJ within a measured tolerance (G-11)

## What to build

A measurement of the core's reprojection (`proj`) against PROJ at the pinned MapServer build, over
the transformations the msautotest cases in scope and the built-in codes of `adapter-epsg` use, and
from it the tolerance in metres that G-11 states. The reference coordinates PROJ computes are
recorded beside the test, as the label records of ADR-0033 are, so that the test needs no PROJ to
run. A result in degrees is compared by its distance in metres on the ellipsoid. Running PROJ on CI
is a run beside MapServer's that ADR-0033 may need revising for.

Ticket 01 ported longitude and latitude, `lcc`, `merc`, and the datum shift, which the case it
brought into scope needs; `tmerc` with `+zone`, which the UTM zones of `adapter-epsg` need, is
not ported yet, and this ticket ports it from PROJ 9.4.0's `src/projections/tmerc.cpp` before it
measures them.

## Acceptance criteria

- [ ] Given the transformations of the cases in scope and of EPSG:4326, EPSG:3857, and the UTM
  zones, when `proj` transforms the recorded points, then each lies within the tolerance of the
  PROJ result, and the test cites `Verifies: G-11`.
- [ ] Given the measurement, when the tolerance is set, then the human decides it and lands it in
  G-11 as a change to the specification, with the measured spread.
- [ ] Given EPSG:3857 as `adapter-epsg` builds it in, with `+nadgrids=@null`, when `proj` reads it,
  then it transforms without a grid file, or the definition changes with the human's decision.
- [ ] Given a recorded reference, when it is read, then a script names the PROJ version and the
  MapServer build that computed it, and rerunning the script reproduces the file.

## Read first

- Touches: `crates/proj` (with `tmerc.rs`), `scripts/build-mapserver.sh`,
  `.github/workflows/noise.yml`, `crates/cli/tests/conformance/`, `docs/SPECIFICATION.md` (G-11,
  landed by the human)
- ADR-0033 (how MapServer's own runs are recorded); ADR-0002 (`Verifies:`)
- Seam: a test of `crates/proj` over recorded points

## Blocked by

- docs/plan/reprojection/01-reproject-features.md

## Mode

with the human: the human sets the tolerance from the measurement.
