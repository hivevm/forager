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
  they include; the fetch checks that the tag still names the pinned commit, sets the sparse
  patterns again on every run, and the CI cache of the suite is keyed on the fetch script. The G-1
  and G-3 tests run over it; they are ignored by `cargo test` and run by the Rust checks, which
  name the suite. They stay as they are.
- `forager render` binds GeoJSON files to layers named on the command line. That stays; binding a
  layer from its own `DATA` is added next to it.
- A layer's `DATA` reaches the style IR as a source reference of kind `local`. `SHAPEPATH` is a
  known keyword that nothing reads yet.
- The engine renders with one source type for all layers; the CLI binds GeoJSON only.
- Decoding a PNG accepts only 8-bit RGBA, which is what G-12 needs.
- The engine renders points, lines, and polygons; its tests pin that, with GeoJSON fixtures.
- No test runs code built for `wasm32-unknown-unknown`; the build for it is checked, and the size
  of the bindings.

## Goals served

- G-6, for the category geometry and symbols.
- G-9, and Q-2 with it.
- Q-4.
- G-8, Q-3, Q-8, and Q-9 are touched: a shapefile source that fails is skipped with a diagnostic
  and without a panic, the files it names resolve inside the base context, and a data file past
  the limit is a diagnostic.

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
  resource loader, with a reader derived from MapServer's, as an ADR of its own decides.
  The frontend reads `SHAPEPATH` and joins it with a `local` source's `DATA` into that source's
  `data`, relative to the mapfile's directory, so that the style document's format does not change;
  a `DATA` without an extension names the `.shp`, as in MapServer. The path resolves inside the
  base context: a `..` that stays inside it, as `../misc/data` does from the suite's `renderers`,
  is allowed; one that leaves it is a diagnostic. The CLI binds a layer of kind `local` to the
  shapefile adapter, beside the GeoJSON files it binds by name, so its sources are of more than one
  adapter.
- **The suite.** The fetch takes, beyond the mapfiles, the shapefiles the cases in `misc` and
  `renderers` read, wherever in the suite they lie (`misc`, `query`, `wxs`), and the non-cairo PNG
  references of those two directories, at the same pinned commit. Nothing of the suite is
  committed.
- **Case.** A case is one `RUN_PARMS` line of a mapfile that runs `[MAP2IMG]` and writes a PNG
  with the output format `png`, or none; the reference is the file of that name under `expected`.
  A `[MAPSERV]` line is an OGC or CGI request, a Non-Goal. The options `-l`, `-e`, and `-s` of
  `map2img` are applied by the harness to the style IR before it renders; any other option puts
  the case out of scope.
- **Scope.** A list in the tree names each case the harness renders, its category, and its
  expected outcome; every other case of `misc` and `renderers` is named with the reason it is out
  of scope (an OGC request, another output format, a `map2img` option not applied, a raster
  source, labels, a projection, a runtime substitution, a feature not built yet). A case moves into
  scope by an edit of that list in the change that makes it pass.
- **Metric.** The rendering and the reference are compared as RGBA buffers of the same size. A
  pixel differs when one of its channels differs by more than the category's channel threshold;
  a case passes when the share of differing pixels is at most the category's tolerance. A
  reference image of another mode, palette or RGB, is expanded to RGBA first, by the `png` crate
  in the harness; the encoder's own decoding stays as G-12 needs it.
- **Tolerance.** The threshold and the tolerance of the category are set from a measurement: the
  pinned MapServer renders the in-scope cases on CI, outside the dev container, and the largest
  difference it shows against its own references is its noise. The numbers go into G-6 as a
  specification change the maintainer signs. How MapServer runs on CI is decided by an ADR. The
  CI job writes MapServer's renderings into a directory, and the harness compares that directory
  with the references by the same metric. Until the numbers are signed, the harness runs with
  provisional ones and cites no criterion, so that G-6 is held only after the noise is measured,
  as the specification's Assumption asks.
- **Fidelity record.** A release is a tag `v<semver>` on `main`. The share of passing cases is
  recorded in the tree with each release; a check, run by the Rust checks after the harness,
  compares the current share against the record of the last release and fails when it is lower.
- **The second target.** The engine with the reference renderer is built for
  `wasm32-unknown-unknown` and run from a native test over the in-scope cases; its buffers are
  compared byte for byte with the native ones. What runs the module, and which crate builds it,
  is decided by an ADR. The core's freedom from hash maps, clocks, and random sources (Q-2) is
  held by the lint configuration.

## Testing decisions

- The harness is an integration test of the CLI crate beside the G-1 test, skipped like it when
  the suite is not fetched, and run by the Rust checks after the fetch.
- The metric is tested on small synthetic buffers: equal, one channel over the threshold, a
  share at and above the tolerance, a different size, a palette image.
- The shapefile adapter is tested at the port with small fixture files: points, lines, polygons
  with holes, multipart shapes, null shapes, and truncated or inconsistent files: a broken record
  reads as no shape, a broken header is an error of the source, and nothing panics (Q-3). The
  memory rules run in a test binary of their own with a counting allocator. The CLI tests copy the
  GeoJSON ones.
- The fidelity check has a self-test, as every check under `scripts/` does.

## Out of scope

- The categories raster and text of G-6; labels (G-10); reprojection (G-11); pushdown (G-7).
- Output formats other than PNG, and the cairo references.
- FlatGeobuf, GeoTIFF, and any data source other than shapefiles.
- Shapefile index files (`.qix`, `.sbn`), tile indexes, and `.cpg` code pages other than the
  default.
- Moving the pinned MapServer release.

## ADR impact

- Binding: ADR-0009 (the adapter is an `adapter-<kind>` crate; the harness lives in a host's tests),
  ADR-0012 (every new crate and tool), ADR-0015 (the reference renderer), ADR-0017 (the `png`
  crate, which the harness reuses), ADR-0023 (the base context the data resolves in), ADR-0007
  (resource paths, the resource limits, `deny.toml`, and the specification stay with the
  maintainer). ADR-0014 and ADR-0020 are not touched, because `SHAPEPATH` folds into the source's
  `data` and the format does not change.
- New `proposed` ADRs before the tickets that need them: the shapefile reader; how the pinned
  MapServer runs on CI; what runs the `wasm32-unknown-unknown` module in a native test. Should the
  module need a host crate of its own, that ADR supersedes ADR-0009 to add it to the table.
- A specification change: the threshold and the tolerance of the category geometry and symbols in
  G-6, and the metric with them.
