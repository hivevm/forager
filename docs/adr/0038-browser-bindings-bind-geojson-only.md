# ADR-0038: The browser bindings bind only the GeoJSON a page passes, never a shapefile

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/bindings-wasm`: which feature sources it binds to the layers, and its dependencies on `adapter-<kind>` crates

## Context

`forager render` binds a local layer whose `DATA` names a shapefile in the base context to the
shapefile adapter ([ADR-0024](0024-shapefile-reader.md)); the browser bindings take the files the
mapfile names from the page, a `.shp` among them if the page passes one, and still bind only the
GeoJSON the page passes by layer name, so the same mapfile and files draw a `DATA` layer on the
command line and skip it in the browser with a diagnostic (G-8). ADR-0024 leaves "whether a browser
host binds the adapter" out of scope, and no other ADR decides it; a reader of `bindings-wasm`
would take the gap for an omission. G-5 asks the bindings for GeoJSON features, Q-5 caps the
bundle at 3 MB gzipped, and Q-7 keeps every source reference but a credential-free `DATA` away from
the page.

Questions the maintainer settled on acceptance:

1. Is a page that hands in shapefiles a use case the browser must serve, or does a server convert
   them to GeoJSON before they leave it?
2. Would the adapter's size in the bundle, measured, change the answer under Q-5?

The decision answers the first: a server converts shapefiles to GeoJSON before they reach a page.
The second is a reason to revisit, not a condition of this decision.

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

- None beyond G-5, Q-5, and ADR-0024's scope; cheap to reverse once a use case asks.

## Consequences

- Positive: the bundle carries one adapter; the browser's binding stays one path.
- Negative / trade-offs: a mapfile with shapefile layers draws less in the browser than on the
  command line.
- Follow-ups: binding shapefiles in the browser, if question 1 asks for it.

## Enforcement

`crates/arch-test` refuses an `adapter-shapefile` dependency of `bindings-wasm`, and a test of
`bindings-wasm` passes a `.shp` among the page's files for a `DATA` layer and expects the layer
skipped with its diagnostic, each citing this ADR with `Verifies: ADR-0038`.
