# Spec: Conformance harness

## Problem

The maintainer cannot tell how close a rendered map comes to MapServer's. The mapfile frontend
loads the whole msautotest suite (G-1), but no test renders a case and holds the result against
its reference image. Every later piece of the render side, labels, reprojection, raster, pushdown,
would be built without a measure of whether it brings Forager closer to MapServer or further
away. Nor does any test show that the browser draws the same pixels as the CLI, which is what makes
a reference image comparable at all.

## Solution

The maintainer runs the checks and learns, per msautotest case in scope, whether Forager's
rendering matches the reference image within the tolerance of the case's category, and why a case
is out of scope where it is. The share of passing cases is recorded at each release and cannot
fall from one release to the next. Every case in scope is also rendered on
`wasm32-unknown-unknown` and must give the same bytes as the native render.

This spec covers the category *geometry and symbols* only. The other categories of G-6, raster and
text, and the criteria G-7, G-10, and G-11, are later specs that use this harness as their measure.

## Existing behaviour

- The suite is fetched at the pinned release by a sparse checkout of its mapfiles and the files
  they include; the fetch already checks that the tag still names the pinned commit. The G-1 and
  G-3 tests run over it and are skipped without the suite. They stay as they are.
- `forager render` binds GeoJSON files to layers named on the command line. That stays; binding a
  layer from its own `DATA` is added next to it.
- A layer's `DATA` reaches the style IR as a source reference of kind `local`. `SHAPEPATH` is not
  read yet.
- The engine renders points, lines, and polygons; its tests pin that, with GeoJSON fixtures.
- No test runs code built for `wasm32-unknown-unknown`; the build for it is checked, and the size
  of the bindings.

## Goals served

- G-6, for the category geometry and symbols.
- G-9, and Q-2 with it.
- Q-4.
- G-8 and Q-8 are touched: a shapefile source that fails is skipped with a diagnostic, and the
  files it names resolve inside the base context.

## User stories

1. As the maintainer, I want every in-scope msautotest case rendered and compared with its
   reference image, so that I know which cases Forager draws as MapServer does.
2. As the maintainer, I want a failing case to leave its rendering and a difference image beside
   the reference, so that I can see where it differs without running anything by hand.
3. As the maintainer, I want each case outside the scope listed with its reason, so that a case
   is never silently dropped and the share of passing cases means something.
4. As the maintainer, I want the tolerance of a category to lie above the noise MapServer itself
   shows against its references, so that a failure is Forager's and not the platform's.
5. As the maintainer, I want the share of passing cases recorded at each release and a check that
   fails when it falls, so that fidelity only moves forward.
6. As the maintainer, I want each in-scope case rendered natively and on `wasm32-unknown-unknown`
   and compared byte for byte, so that what the browser shows is what the harness measured.
7. As a user with a mapfile over shapefiles, I want `forager render` to read the layers' shapefiles
   from their `DATA` under `SHAPEPATH`, so that I can render my map without converting its data.
8. As a host author, I want a shapefile adapter that reads its files through the resource loader,
   so that a browser host can use it as well as the CLI.

## Implementation decisions

- **Data.** A new adapter reads ESRI shapefiles, the `.shp` with its `.shx` and `.dbf`, through the
  resource loader. The crate it uses is decided by an ADR before the first ticket starts. The
  frontend reads `SHAPEPATH`, and a `local` source resolves its `DATA` against it, inside the base
  context; a `DATA` without an extension names the `.shp`, as in MapServer.
- **The suite.** The fetch takes, beyond the mapfiles, the data and the reference images of the
  directories in scope, at the same pinned commit. Nothing of the suite is committed.
- **Scope.** A list in the tree names each case the harness renders, its category, and its
  expected outcome; every other case of the directories it covers is named with the reason it is
  out of scope (another output format, a raster source, labels, a projection, a feature not built
  yet). A case moves into scope by an edit of that list in the change that makes it pass.
- **Metric.** The rendering and the reference are compared as RGBA buffers of the same size. A
  pixel differs when one of its channels differs by more than the category's channel threshold;
  a case passes when the share of differing pixels is at most the category's tolerance. A
  reference image of another mode, palette or RGB, is expanded to RGBA first.
- **Tolerance.** The threshold and the tolerance of the category are set from a measurement: the
  pinned MapServer renders the in-scope cases on CI, outside the dev container, and the largest
  difference it shows against its own references is its noise. The numbers go into G-6 as a
  specification change the maintainer signs. How MapServer runs on CI is decided by an ADR.
- **Fidelity record.** The share of passing cases is recorded in the tree with each release; a
  check compares the current share against the record of the last release and fails when it is
  lower.
- **The second target.** The engine with the reference renderer is built for
  `wasm32-unknown-unknown` and run from a native test over the in-scope cases; its buffers are
  compared byte for byte with the native ones. What runs the module is decided by an ADR. The
  core's freedom from hash-map iteration order, clocks, and random sources (Q-2) gets a check.

## Testing decisions

- The harness is an integration test of the CLI crate beside the G-1 test, skipped like it when
  the suite is not fetched, and run by the Rust checks after the fetch.
- The metric is tested on small synthetic buffers: equal, one channel over the threshold, a
  share at and above the tolerance, a different size, a palette image.
- The shapefile adapter is tested at the port with small fixture files: points, lines, polygons
  with holes, multipart shapes, null shapes, attributes of each `.dbf` type, and truncated or
  inconsistent files, which become diagnostics (Q-3). The CLI tests copy the GeoJSON ones.
- The fidelity check and the determinism check each have a self-test, as every check under
  `scripts/` does.

## Out of scope

- The categories raster and text of G-6; labels (G-10); reprojection (G-11); pushdown (G-7).
- Output formats other than PNG, and the cairo references.
- FlatGeobuf, GeoTIFF, and any data source other than shapefiles.
- Shapefile index files (`.qix`, `.sbn`), tile indexes, and `.cpg` code pages other than the
  default.
- Moving the pinned MapServer release.

## ADR impact

- Binding: ADR-0009 (the adapter is an `adapter-<kind>` crate; the harness lives in a host's tests),
  ADR-0012 (every new crate and tool), ADR-0015 (the reference renderer), ADR-0023 (the base
  context the data resolves in), ADR-0007 (the resolution of resource paths and the specification
  stay with the maintainer).
- New `proposed` ADRs before the tickets that need them: the shapefile crate; how the pinned
  MapServer runs on CI; what runs the `wasm32-unknown-unknown` module in a native test.
- A specification change: the threshold and the tolerance of the category geometry and symbols in
  G-6, and the metric with them.
