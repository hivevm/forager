# Ticket: proj4rs is held to PROJ within a measured tolerance (G-11)


## What to build

A measurement of proj4rs against PROJ at the pinned MapServer build, over the transformations the
msautotest cases in scope and the built-in codes of `adapter-epsg` use, and from it the tolerance in
metres that G-11 states. The reference coordinates PROJ computes are recorded beside the test, as
the label records of ADR-0033 are, so that the test needs no PROJ to run.

## Acceptance criteria

- [ ] Given the transformations of the cases in scope and of EPSG:4326, EPSG:3857, and the UTM
  zones, when proj4rs transforms the recorded points, then each lies within the tolerance of the
  PROJ result, and the test cites `Verifies: G-11`.
- [ ] Given the measurement, when the tolerance is set, then the human decides it, and the ADR of
  ticket 01 or a revision of it states it with the measured spread.
- [ ] Given a recorded reference, when it is read, then a script names the PROJ version and the
  MapServer build that computed it, and rerunning the script reproduces the file.

## Read first

- Touches: `crates/proj`, `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`,
  `crates/cli/tests/conformance/`
- ADR-0033 (how MapServer's own runs are recorded); ADR-0002 (`Verifies:`)
- Seam: a test of `crates/proj` over recorded points

## Blocked by

- docs/plan/reprojection/01-reproject-features.md

## Mode

with the human: the human sets the tolerance from the measurement.
