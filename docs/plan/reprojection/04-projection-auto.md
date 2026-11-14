# Ticket: `PROJECTION AUTO` takes the projection the source declares


## What to build

A layer of `PROJECTION AUTO` takes its projection from its source: a shapefile from the WKT of the
`.prj` beside it, turned into a proj4 string in the shapefile adapter, and a GeoJSON source as WGS84
longitude and latitude. A source that declares none leaves the layer as `msProjectCreateReprojector`
leaves it: longitude and latitude in the datum of the map's projection. A map of `PROJECTION AUTO`
has no projection, as `msProcessProjection` gives it, and the frontend no longer warns of it as not
yet supported. Before any code, `propose-adr` runs for the projection a source declares: the
change to the feature source port, and the crate that turns WKT into proj4 (proj4wkt or another).

## Acceptance criteria

- [ ] Given a proposed ADR for the projection a source declares, reviewed by the human, when the
  work starts, then it settles the port's change under ADR-0022 and ADR-0023 and the WKT crate under
  ADR-0011.
- [ ] Given a shapefile layer of `PROJECTION AUTO` with a `.prj` in a projection proj4rs supports,
  when it is rendered under a map in another projection, then it is reprojected from the `.prj`'s
  projection.
- [ ] Given a `.prj` that does not read or names a projection the conversion does not support, when
  the layer is rendered, then it warns naming the layer and the file, and is treated as declaring
  none.
- [ ] Given a GeoJSON layer of `PROJECTION AUTO`, when it is rendered under a map in UTM, then it is
  reprojected from WGS84 longitude and latitude, in the CLI and in the browser bindings (G-5).
- [ ] Given a map of `PROJECTION AUTO`, when it is loaded, then the style IR holds no map projection
  and no `unsupported-value` warning is reported.

## Read first

- Touches: `crates/ports/src/lib.rs`, `crates/adapter-shapefile`, `crates/adapter-geojson`,
  `crates/engine/src/lib.rs`, `crates/mapfile/src/translate/mod.rs`, `crates/bindings-wasm`
- `docs/semantics/projection.md` (**AUTO**), `docs/semantics/shapefile.md`; MapServer's
  `msSHPLayerOpen` (`src/mapshape.c`) and `msProcessProjection` (`src/mapproject.c`)
- Glossary: Adapter, Feature source, Port
- Seam: the frontend's `load` with files in memory; the shapefile adapter's tests with fixtures; the
  engine's render with sources in memory

## Blocked by

- docs/plan/reprojection/01-reproject-features.md

## Mode

with the human: it starts with an ADR that changes a port.
