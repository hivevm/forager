# Ticket: A WKT polygon's hole that crosses itself is skipped

## What to build

The frontend's WKT reader skips an inner ring of a `POLYGON` or a `MULTIPOLYGON` that GEOS does
not call a ring, as `msGEOSGeometry2Shape` (`src/mapgeos.c`) does where `GEOSisRing` rejects it:
a ring that is not simple, one whose edges meet anywhere but at consecutive edges' shared vertex
and at its closing point. The reader keeps such a ring today, a known divergence named in
`docs/semantics/feature.md`. The test of simplicity keeps to Q-9: a sweep of the ring's edges in
O(n log n), not the test of every pair of edges, so that a hostile hole of many vertices costs no
more than reading it.

## Acceptance criteria

- [ ] Given a `POLYGON` whose inner ring crosses itself, as a bow tie, when the mapfile loads, then
      the feature's parts are its exterior and its other inner rings, the crossing one left out,
      and the same of each polygon of a `MULTIPOLYGON`.
- [ ] Given an inner ring that is simple, one that touches its exterior at a vertex included, when
      the mapfile loads, then it is kept, as GEOS keeps it.
- [ ] Given an exterior ring that crosses itself, when the mapfile loads, then it is kept, as
      `msGEOSGeometry2Shape` tests only the inner rings.
- [ ] Given an inner ring of a million vertices, when the mapfile loads, then the test's time and
      memory grow in O(n log n) of its vertices, bounded by a test that would fail the naive
      pairwise test.
- [ ] Given the change, then `docs/semantics/feature.md` no longer lists the divergence, and the
      fuzz target of the mapfile frontend reads the new path without a panic or a hang.

## Read first

- `docs/adr/0033-inline-features-in-the-style-ir.md`, its WKT clause.
- `crates/mapfile/src/wkt.rs`, its `polygon`, and its tests; `docs/semantics/feature.md`.
- MapServer at the pin: `msGEOSGeometry2Shape_polygon` and `msGEOSGeometry2Shape_multipolygon`
  in `src/mapgeos.c`; what GEOS's `isRing` and `IsSimpleOp` accept for a linear ring, read as a
  specification of the result, re-implemented without GEOS code.
- `docs/CONVENTIONS.md`, the entries on ported behaviour and on readers of untrusted bytes.

## Blocked by

- None (can start immediately)

## Mode

unattended: the frontend's tests decide every criterion, the cost criterion by a bound a pairwise
test would break; the change stays in `crates/mapfile/src/wkt.rs`, its tests, and the semantics
page, undone by one revert; nothing ADR-0007 keeps with the maintainer, and it counts against
ADR-0007's cap on open agent branches. Stop, write what was found here, and hand off when GEOS's
notion of a simple ring cannot be stated from its documented behaviour and the suite without
reading its code, or when no sweep within O(n log n) decides it after two attempts.
