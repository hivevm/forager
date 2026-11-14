# Ticket: A map's `PROJECTION AUTO` is read as MapServer reads it

## What to build

A map of `PROJECTION AUTO` is read as `msProcessProjection` reads it: `AUTO` stays the map's one
argument, without a PROJ object. A layer without a projection is then not reprojected, and a layer
in a projection differs from the map (`msProjectionsDiffer`) and is reprojected into its base
geographic CRS (`msProjectCreateReprojector`). The style IR tells a map of `AUTO` from a map without
a projection, which changes the style document: the change takes the format version ADR-0014 asks
for, with its migration where it is a major one. The frontend no longer warns of `AUTO` as not yet
supported.

## Acceptance criteria

- [ ] Given a map of `PROJECTION AUTO`, when it is loaded, then the style IR tells it from a map
  without a projection, and no `unsupported-value` warning is reported.
- [ ] Given the changed map projection, when the style document is written and read, then G-3's
  round trip holds, the format version moves as ADR-0014 decides, the checked-in schema follows,
  and a document of the former version reads as before (G-4).
- [ ] Given a map of `PROJECTION AUTO` and a layer in UTM, when it is rendered, then the layer is
  drawn in the longitude and latitude of its own datum, as MapServer draws it; a layer without a
  projection beneath it is drawn unreprojected.

## Read first

- Touches: `crates/style-ir` (the map's projection, `migration.rs` if the version is major),
  `schema/`, `crates/mapfile/src/translate/mod.rs`, `crates/engine/src/lib.rs`,
  `docs/semantics/projection.md` (**AUTO**)
- MapServer's `msProcessProjection`, `msProjectionsDiffer`, and `msProjectCreateReprojector`
  (`src/mapproject.c`) at the pinned tag
- Glossary: Style document, Format version, Style IR
- Seam: the frontend's `load` with files in memory; `crates/style-ir/tests/schema.rs`; the engine's
  render with sources in memory

## Blocked by

- docs/plan/reprojection/01-reproject-features.md

## Mode

with the human: it changes the style document's format, whose version the human releases.
