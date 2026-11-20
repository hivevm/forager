# API-source-declares-projection: A feature source declares its projection through the port, and the shapefile adapter turns the WKT of its `.prj` into a proj4 string with a parser of its own

- **Status:** 🟢 accepted
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the `FeatureSource` port of `crates/ports` and its declared projection, its use by a layer of `PROJECTION AUTO` in `crates/engine`, the `.prj` reader of `crates/adapter-shapefile` and its fuzz target `fuzz/fuzz_targets/prj.rs`, and the declaration of `crates/adapter-geojson`

## Context

A layer of `PROJECTION AUTO` takes its projection from its source, as MapServer's
`msSHPLayerOpen` (`src/mapshape.c`) does for a shapefile. When the layer's first argument is
`auto`, it reads up to 2047 bytes of the `.prj` beside the `.shp`. GDAL's OGR parses them as ESRI
WKT (`OSRNewSpatialReference`, `OSRMorphFromESRI`). `msOGRSpatialRef2ProjectionObj`
(`src/mapogr.cpp`) then turns the result into the layer's projection:
- a CRS with an EPSG authority becomes `init=epsg:<code>`;
- a `LOCAL_CS` becomes no projection;
- any other CRS becomes the proj4 string `OSRExportToProj4` writes.

Two failures leave different states:
- **The `.prj` is missing or does not parse:** the layer keeps `AUTO` without a PROJ object.
  `msProjectCreateReprojector` (`src/mapproject.c`) then takes its coordinates as longitude and
  latitude in the datum of the map's projection (`getBaseGeographicCRS`).
- **The conversion fails after the parse:** the layer is left with no projection, and nothing
  reprojects.

MapServer writes only a debug message in either case. GeoJSON is longitude and latitude in WGS84
by RFC 7946. In MapServer that is OGR's declaration, the same as `+proj=longlat +datum=WGS84`.

The forces:
- The core accepts proj4 strings only (spec, *Constraints*), so WKT must become proj4 outside it,
  in the adapter (Q-1, [ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md)).
- The engine needs the projection before it asks the source for features. The area it asks for is
  in the source's coordinates (the plan's ticket 03).
- The port's features carry no projection today
  ([API-skipped-records-reported](API-skipped-records-reported.md),
  [API-typed-feature-properties](API-typed-feature-properties.md)).
- Every crate added is weighed under [SEC-crates-locked-and-denied](SEC-crates-locked-and-denied.md)
  and against Q-5's bundle.
- `proj` implements only longitude and latitude, `lcc`, `merc`, `tmerc` with `utm`, and `ortho`
  ([GEO-proj-ported-with-libm](GEO-proj-ported-with-libm.md)), on WGS84 and NAD83 in metres; ESRI's
  `Orthographic` is spherical, which `ortho` of a datum is not. A WKT converted to anything else
  could not be reprojected anyway.
- A `.prj` is untrusted input (Q-3): its reader must never panic, and the weekly fuzz run feeds
  it.

The maintainer chose a parser of the adapter's own over the crate `proj4wkt` on 2026-11-15.

## Decision

We will let a feature source declare its projection through a synchronous method of the
`FeatureSource` port, with a default that declares none, and have the shapefile adapter read its
`.prj` at open and turn the WKT into a proj4 string with a parser of its own, limited to what
`proj` implements.

- **The port.** `FeatureSource` gains `fn declared_projection(&self) -> DeclaredProjection`. Its
  default returns `DeclaredProjection::None`. `DeclaredProjection` has four variants:
  - `None`: no declaration;
  - `Proj4(String)`: a proj4 string, as the style IR holds one;
  - `NoProjection`: MapServer's empty projection, for a `LOCAL_CS` or a conversion that fails
    after the parse;
  - `Unreadable { file, why }`: a declaration that does not read.
  It is synchronous, because the adapter has read what it needs at open, as `msSHPLayerOpen`
  reads at open.
- **The shapefile adapter.**
  - `ShapefileSource::open` loads the `.prj` beside the `.shp`, its name with the extension `prj`
    in lower case only, as MapServer's `CPLResetExtension` names it. A missing `.prj` is no error,
    and the adapter keeps the first 2047 bytes.
  - `declared_projection` parses them as ESRI or OGC WKT, `GEOGCS` and `PROJCS` with their
    `DATUM`, `SPHEROID`, `PRIMEM`, `PROJECTION`, `PARAMETER`, `UNIT`, and `AXIS`. It writes the
    proj4 string `OSRExportToProj4` writes for the projections, datums, and units `proj`
    implements.
  - An `AUTHORITY["EPSG", …]` on the CRS is not followed. MapServer turns it into
    `init=epsg:<code>`; the adapter writes the proj4 string from the parameters instead, because the
    render has no projection resolver (API-init-projections-through-resolver keeps it at load).
    Where PROJ's EPSG entry and the parameters agree, the coordinates agree too; the strings differ.
  - Anything the parser does not read or support is `Unreadable`, naming the file and why.
- **The GeoJSON adapter** declares `Proj4("+proj=longlat +datum=WGS84 +no_defs")`.
- **The engine.** For a layer of `PROJECTION AUTO` it asks the bound source:
  - `Proj4` reprojects from it as from a layer's own projection;
  - `NoProjection` reprojects nothing;
  - `None` and `Unreadable` keep `AUTO`, reprojected from longitude and latitude in the datum of
    the map's projection, as `msProjectCreateReprojector` does;
  - `Unreadable` also warns, naming the layer and the file. MapServer writes only a debug message
    there; Forager has no debug log, so it warns by intent (Q-3).

**Out of scope:**
- WKT2 and PROJJSON;
- GeoJSON's legacy `crs` member, which RFC 7946 removed;
- FlatGeobuf's declared CRS, which comes with its adapter;
- a raster's projection;
- any projection, datum, or unit `proj` does not implement. Those widen with `proj`.

## Alternatives considered

- **The crate `proj4wkt` (3liz, MIT OR Apache-2.0).** It reads more WKT and is maintained with
  proj4rs. Not chosen:
  - it adds a dependency to the browser bundle for conversions `proj` could not compute;
  - its output follows proj4js rather than `OSRExportToProj4`, so Forager's strings and MapServer's
    would differ beyond the cases in scope;
  - the maintainer preferred a parser whose reach is exactly `proj`'s.
- **WKT handed to the core, which turns it into a projection itself.** It would keep one parser
  for every adapter, but the spec's *Constraints* keep the core to proj4 strings. It would also
  put a text format into `proj` that only some sources need.
- **The projection on `Features`, returned with each query.** It needs no new method, but the
  engine needs the projection before its first query, since the area it asks for is in the
  source's coordinates.
- **An EPSG authority resolved as MapServer resolves it.** It would be closest to MapServer, but
  it needs the projection resolver in every render and in every host's binding of sources. An
  ESRI `.prj` rarely carries an authority, and the parameters give the same coordinates for the
  codes in scope.
- **An `async` method.** It would allow a source that fetches its declaration lazily, but no
  source in scope does, and `msSHPLayerOpen` reads at open. A future source can read at its own
  open.
- **The host resolves `AUTO` before the render, rewriting the layer's projection in the style
  IR.** It would leave the port unchanged, but every host would repeat the rule for `AUTO`. The
  style document would also carry a projection the mapfile does not state (G-3).
- **A warning-free fallback, as MapServer's debug message.** It would be closer to MapServer's
  output, but a layer drawn in the wrong place without a word goes against Q-3.

## Sources / Prior art

- MapServer at the pin: `msSHPLayerOpen` in `src/mapshape.c`; `msOGCWKT2ProjectionObj` and
  `msOGRSpatialRef2ProjectionObj` in `src/mapogr.cpp`; `msProjectCreateReprojector` and
  `getBaseGeographicCRS` in `src/mapproject.c`.
- GDAL's OGR spatial reference: `OSRMorphFromESRI`, `OSRExportToProj4`
  (<https://gdal.org/api/ogrspatialref.html>), and the ESRI WKT dialect of `.prj` files.
- OGC 01-009, *Coordinate Transformation Services* (the WKT1 grammar the `.prj` follows).
- RFC 7946, section 4 (GeoJSON's coordinate reference system).
- `proj4wkt` (<https://github.com/3liz/proj4wkt-rs>), the alternative.

## Consequences

- Positive:
  - a shapefile of `PROJECTION AUTO` is drawn where MapServer draws it, for the projections
    `proj` implements;
  - GeoJSON under any map projection is drawn as RFC 7946 data, in the CLI and in the browser
    (G-5);
  - no dependency is added.
- Negative / trade-offs:
  - every implementation of the port gets a default it may need to override;
  - the parser follows `OSRExportToProj4` only as far as `proj` reaches, and must widen with it;
  - the `.prj` is loaded at open for every shapefile, of a layer of `AUTO` or not, since a source
    does not know its layer: one more load per shapefile, a fetch in the browser;
  - a `.prj` that MapServer reads but Forager does not support is drawn in the map datum's
    longitude and latitude, with a warning. MapServer draws it in its projection.
- Follow-ups:
  - the declared projection of a FlatGeobuf source;
  - the WKT a WMS or WFS source might declare, should one come into scope.

## Enforcement

- `forager_ports`: a test that a source without an override declares `None`.
- `crates/adapter-shapefile`: tests with `.prj` fixtures. They cover:
  - a WGS84 `GEOGCS`, a UTM and a Lambert conformal conic `PROJCS`, and one with an EPSG
    authority, written from its parameters;
  - a `LOCAL_CS`, a `.prj` past 2047 bytes, and one naming an unsupported projection;
  - a missing `.prj`.
  Each cites this ADR with `Verifies:`.
- `crates/engine`: renders with sources in memory, for each variant of `DeclaredProjection` under
  a layer of `AUTO`, citing this ADR.
- `fuzz/`: a target feeds the `.prj` parser arbitrary bytes, run weekly (Q-3).
