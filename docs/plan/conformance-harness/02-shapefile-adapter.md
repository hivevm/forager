# Ticket: Read shapefiles at the feature source port

## What to build

A new crate `adapter-shapefile` with a reader derived from MapServer's `src/mapshape.c` and
`src/mapxbase.c`, as [ADR-0024](../../adr/0024-shapefile-reader.md) decides: a feature source that
reads a `.shp` with its `.shx` and `.dbf` through the resource loader and yields features with
their attributes, in the shape the GeoJSON adapter yields them. Implemented while that ADR is
`proposed` ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4).

## Acceptance criteria

- [ ] Given fixture shapefiles of points, multipoints, lines, polygons with a hole, multipart
      shapes, and a null shape, when the source is queried with an extent, then it returns the
      features inside it with their geometry, the null shape skipped.
- [ ] Given a `.dbf` with a field of each type (character, numeric, float, logical, date), when
      the source is queried, then each attribute is a property of the matching kind.
- [ ] Given a record past one of MapServer's guards — more than 50,000,000 points or 10,000,000
      parts, 2^28 points in a 48-byte record, a part without points, a part out of order — when
      the source is queried, then that record is read as no shape, the next record is read, and
      nothing beyond the record's size is allocated.
- [ ] Given a truncated `.shp`, when the source is queried, then the records it still holds are
      read and the rest are no shape; given a `.shx` with a short header or a missing `.dbf`, then
      it returns an error of the source; nothing panics (Q-3).
- [ ] Given a `.dbf` whose language driver names a code page, whose update date is zero, or whose
      text holds a byte that is not UTF-8, when the source is queried, then it reads, the byte
      replaced.
- [ ] A test cites ADR-0024 with a `Verifies:` marker, and every file derived from MapServer's
      passes `scripts/check-notice.sh`.

## Read first

- ADR-0024 and the MapServer functions its Sources name, at the pin; ADR-0009 (an
  `adapter-<kind>` crate and its edges); the licence constraint and `scripts/check-notice.sh`.
- Glossary: *adapter*, *port*, *resource loader*, *property*.
- Golden path: the GeoJSON adapter and its tests; the seam is the feature source port.

## Blocked by

- docs/plan/conformance-harness/01-shapefile-crate-adr.md

## Mode

unattended: tests at the port decide every criterion; one new crate that nothing yet binds, undone
by one revert; it changes no port trait, no resource path resolution, no limit, nothing else
ADR-0007 keeps with the maintainer. A wrong result shows as a fixture feature read wrong. It counts
against the cap of three open agent branches of [ADR-0007](../../adr/0007-harness.md), and is
dispatched only while that cap allows. Stop when a criterion needs a change to the port traits, or
a guard of MapServer's that no fixture can reach, and hand off.
