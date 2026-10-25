# Ticket: Read shapefiles at the feature source port

## What to build

A new crate `adapter-shapefile` on the crate the ADR of ticket 01 chooses: a feature source that
reads a `.shp` with its `.shx` and `.dbf` through the resource loader and yields features with
their attributes, in the shape the GeoJSON adapter yields them. Implemented while that ADR is
`proposed` ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4).

## Acceptance criteria

- [ ] Given fixture shapefiles of points, multipoints, lines, polygons with a hole, multipart
      shapes, and a null shape, when the source is queried with an extent, then it returns the
      features inside it with their geometry, the null shape skipped.
- [ ] Given a `.dbf` with a field of each type (character, numeric, float, logical, date), when
      the source is queried, then each attribute is a property of the matching kind.
- [ ] Given a truncated `.shp`, a `.shx` that disagrees with it, or a missing `.dbf`, when the
      source is queried, then it returns an error of the source, and nothing panics (Q-3).
- [ ] A test cites the ADR of ticket 01 with a `Verifies:` marker.

## Read first

- ADR-0009 (an `adapter-<kind>` crate and its edges), ADR-0012, ADR-0016, and the ADR of
  ticket 01.
- Glossary: *adapter*, *port*, *resource loader*, *property*.
- Golden path: the GeoJSON adapter and its tests; the seam is the feature source port.

## Blocked by

- docs/plan/conformance-harness/01-shapefile-crate-adr.md

## Mode

unattended: tests at the port decide every criterion; one new crate that nothing yet binds, undone
by one revert; it changes no port trait, no resource path resolution, no limit, nothing else
ADR-0007 keeps with the maintainer. A wrong result shows as a fixture feature read wrong. It counts
against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is
dispatched only while that cap allows. Stop when a criterion needs a change to the port traits or to
the chosen crate, and hand off.
