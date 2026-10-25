# Ticket: Carry a feature's properties through the source port

## What to build

A feature as a source delivers it carries its properties, so that a class's `EXPRESSION`, a
binding, and a label's text can read them later: the GeoJSON adapter fills them from `properties`,
the shapefile adapter from the `.dbf`, as `docs/semantics/shapefile.md` reads its fields. How a
property's value is kept is decided here, so that a shapefile's attributes cost no more than its
`.dbf` record and a field table its layer shares, as ADR-0024 requires.

## Acceptance criteria

- [ ] Given a GeoJSON feature with a string, a number, a boolean, and a null property, when the
      source is queried, then the feature carries each, of its kind.
- [ ] Given a `.dbf` with a field of each type MapServer reads, when the shapefile source is
      queried, then each feature carries each field's value as MapServer's `msDBFReadAttribute`
      reads it, trimmed as it trims.
- [ ] Given a shape past the `.dbf`'s records, then its feature carries no properties and draws.
- [ ] Given a `.dbf` header of many zero-width fields over many records, when the source is
      queried under the counting allocator of ticket 02, then the peak allocation stays under the
      stated multiple of the files' size.
- [ ] The engine's tests and the CLI's render tests stay green unchanged.

## Read first

- ADR-0007 (the port traits stay with the maintainer), ADR-0009, ADR-0016, ADR-0019 (the
  expressions that will read the properties), ADR-0024.
- Glossary: *property*, *feature*, *port*; `docs/semantics/shapefile.md`.

## Blocked by

- docs/plan/conformance-harness/02-shapefile-adapter.md

## Mode

with the human: it changes a port trait's types, which the harness keeps with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
