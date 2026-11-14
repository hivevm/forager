# Spec: Reprojection of vector layers


## Problem

A mapfile often holds layers in other projections than the map's: a shapefile in a national grid
under a map in Web Mercator, GeoJSON in longitude and latitude under a map in UTM. MapServer
reprojects each such layer into the map's projection. Forager reads every `PROJECTION` but
reprojects nothing, so such a layer is drawn at the wrong place or not at all, and the source is
asked for the map's extent in the wrong coordinates.

## Solution

A layer whose projection differs from the map's is drawn where MapServer draws it, in the browser
and on the server alike: its features are reprojected into the map's projection, and its source is
asked for the area in view in the source's own coordinates. A layer of `PROJECTION AUTO` takes its
projection from its source as MapServer does: a shapefile from the `.prj` beside it, GeoJSON as
longitude and latitude in WGS84. A map of `PROJECTION AUTO` has no projection, as in MapServer.

## Existing behaviour

- The frontend reads the map's and each layer's `PROJECTION` into the style IR as a proj4 string,
  resolves `init=` through the projection resolver or an init file (ADR-0019), and keeps a layer's
  `AUTO` as `from_source`. The tests of the frontend pin this; it stays.
- The engine asks each source for the map's extent, whatever the layer's projection, although the
  feature source port asks for the area "in the source's coordinates". This changes.
- A layer without a projection, or in the map's, is drawn as today: no reprojection, no cost. The
  conformance cases in scope pin it and must stay as they are.
- A map's `AUTO` is warned as not yet supported and leaves the map without a projection. The
  projection stays absent; the warning goes.
- The conformance list leaves out every msautotest case that needs a projection, with the reason
  "a projection". Each ticket brings its cases into scope.

## Goals served

- G-11 (proj4rs agrees with PROJ within a tolerance stated in metres; no test cites it yet)
- G-6, Q-4 (the cases of misc and renderers that need a projection come into scope)
- G-9, Q-2 (reprojected renders are bit-identical natively and on wasm32)
- Q-1 (the reprojection is pure Rust and builds for wasm32), Q-5 (bundle size), Q-6 (licence)
- G-5 (the browser bindings reproject as the CLI does)

## User stories

1. As a cartographer, I want a layer in another projection drawn where MapServer draws it, so that
   my mapfile needs no change to be shown in the browser.
2. As a cartographer, I want a shapefile of `PROJECTION AUTO` to take its projection from its
   `.prj`, so that the mapfile does not repeat what the data declares.
3. As a web developer, I want GeoJSON layers taken as WGS84 longitude and latitude, so that a page
   can pass RFC 7946 GeoJSON under any map projection.
4. As a host developer, I want the source asked for the area in view in its own coordinates, so
   that a source that filters by extent returns the right features.
5. As the maintainer, I want proj4rs measured against PROJ at the pinned MapServer, so that G-11
   states a tolerance the measurement supports.
6. As a cartographer, I want lines crossing the dateline and maps in an orthographic projection
   drawn as MapServer draws them, so that world maps keep their shape.

## Implementation decisions

- The reprojection lives in the core crate `proj`, which ADR-0008's table already names; the
  engine reprojects a feature's shape after the source delivers it and before its layer's
  `GEOMTRANSFORM`, classes, and labels, as `msDrawShape` reprojects before it draws.
- The area asked of a source is the map's extent projected into the layer's projection as
  `msProjectRect` projects it.
- A layer projects only where its projection and the map's differ as `msProjectionsDiffer` finds
  it; a layer or a map without one projects nothing.
- WKT from a `.prj` is turned into a proj4 string in the shapefile adapter, outside the core, which
  accepts proj4 only (*Constraints*). The feature source port carries the projection a source
  declares. A GeoJSON source declares WGS84 longitude and latitude.
- The tolerance of G-11 is set from a measurement of proj4rs against PROJ at the pinned MapServer
  build, as the noise of G-6 was.
- Horizon clipping into an orthographic projection (MapServer's RFC 5) and the dateline handling of
  `+over` are their own late tickets.

## Testing decisions

- The msautotest harness at the seam it has: each case that needs a projection flips from out to in
  scope in the ticket that draws it (`quoting`, `dateline`, `ortho`, `bug1526`), holding its outcome
  and the G-9 comparison on wasm32.
- The frontend's `load` with files in memory for `AUTO` of the map and the layer.
- The engine's render with sources in memory for the area asked of a source.
- G-11 by a test with reference coordinates PROJ computed at the pinned build, recorded beside the
  test as the label records of ADR-0033 are.
- The shapefile adapter's tests for `.prj` with fixture files.

## Out of scope

- Raster reprojection (the raster package).
- Datum grids, a mapfile's `PROJECTION` given as WKT, PROJJSON, a PROJ pipeline, or `EPSG:` without
  `init=` (Non-Goals as drafted with the function matrix).
- An EPSG database in the core; `init=` stays with the resolver (ADR-0019).
- FlatGeobuf's declared projection, which comes with the FlatGeobuf adapter.
- Map rotation (`ANGLE`), which interacts with the extent but is its own package.

## ADR impact

- Binding: ADR-0008 (the `proj` crate and its edges), ADR-0019 (proj4 strings from the resolver),
  ADR-0011 (a new dependency), ADR-0022 and ADR-0023 (the port and its features), ADR-0024 (the
  browser bindings' sources), ADR-0034 (G-9 on wasm32), ADR-0002 (`Verifies: G-11`).
- New `proposed` ADRs before implementation:
  - the reprojection with proj4rs in `proj`: the dependency, its build for wasm32 without C code,
    its bundle size, and how its trigonometry stays bit-identical across targets where std's calls
    the platform's libm (ticket 01);
  - the projection a source declares: the port's change, and the WKT conversion in the shapefile
    adapter with its crate (ticket 04).
