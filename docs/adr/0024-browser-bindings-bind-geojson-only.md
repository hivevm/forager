# ADR-0024: The browser bindings bind only the GeoJSON a page passes, never a shapefile

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/bindings-wasm`: which feature sources it binds to the layers, and its dependencies on `adapter-<kind>` crates

## Context

`forager render` binds a local layer whose `DATA` names a shapefile in the base context to the
shapefile adapter ([ADR-0021](0021-shapefile-adapter.md)). The browser bindings take the files the
mapfile names from the page, a `.shp` among them if the page passes one, and bind only the GeoJSON
the page passes by layer name. The same mapfile and files therefore draw a `DATA` layer on the
command line and skip it in the browser with a diagnostic (G-8). G-5 asks the bindings for GeoJSON
features per layer, Q-5 caps the bundle at 3 MB gzipped, and Q-7 keeps every source reference but
a credential-free `DATA` away from the page. A reader of `bindings-wasm` without a decision would
take the gap for an omission.

The maintainer settled two questions. A page that hands in shapefiles is no use case the browser
must serve: a server converts shapefiles to GeoJSON before they reach a page. The adapter's size
in the bundle, measured, is a reason to revisit, not a condition of this decision.

## Decision

We will have the browser bindings bind only the GeoJSON a page passes by layer name, and leave a
layer whose source is a shapefile unbound, so that the engine skips it with its diagnostic.

**Out of scope:** other kinds of source in the browser; how a server host prepares GeoJSON for a
page.

## Alternatives considered

- **Bind the shapefile adapter over the page's files** — the CLI's behaviour in the browser, at
  the cost of the adapter in every bundle and of a second binding path to keep alike with the
  CLI's.
- **Refuse a `.shp` among the page's files** — a clearer message, and a check on input that G-8's
  diagnostic already gives per layer.

## Sources / Prior art

- None beyond G-5, Q-5, and the scope of [ADR-0021](0021-shapefile-adapter.md); cheap to reverse
  once a use case asks.

## Consequences

- Positive: the bundle carries one adapter, the GeoJSON one
  ([ADR-0020](0020-geojson-adapter.md)); the browser's binding stays one path.
- Negative / trade-offs: a mapfile with shapefile layers draws less in the browser than on the
  command line.
- Follow-ups: binding shapefiles in the browser, if a page that hands in shapefiles becomes a use
  case.

## Enforcement

The test of `crates/arch-test/tests/dependencies.rs` refuses an `adapter-shapefile` dependency of
`bindings-wasm`. The test `a_shapefile_among_the_pages_files_leaves_its_layer_skipped` of
`crates/bindings-wasm/src/lib.rs` passes a `.shp` among the page's files for a `DATA` layer and
expects the layer skipped with its diagnostic. Each cites this ADR with `Verifies:`.
