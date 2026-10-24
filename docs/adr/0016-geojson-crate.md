# ADR-0016: The GeoJSON adapter parses with the georust `geojson` crate

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `crates/adapter-geojson`

## Context

The browser example renders GeoJSON (G-5), and the CLI renders a layer from a GeoJSON file. The
adapter reads RFC 7946 documents — features, the seven geometry types, and properties — into the
features the ports deliver, without panicking on malformed input (Q-3), and builds for
`wasm32-unknown-unknown`.

## Decision

We will parse GeoJSON with the georust `geojson` crate, without its `geo-types` feature, in
`adapter-geojson`.

**Out of scope:** the shape of the features the ports deliver; GeoJSON output.

## Alternatives considered

- **Our own parsing with `serde_json`** — no further dependency, but RFC 7946's geometry types,
  nesting, and foreign members written again.

## Sources / Prior art

- `geojson` 1.0.0, <https://github.com/georust/geojson> (MIT OR Apache-2.0), checked 2026-10-23:
  builds for `wasm32-unknown-unknown`, depends on serde, `serde_json`, `log`, and `thiserror`.
- RFC 7946, <https://www.rfc-editor.org/rfc/rfc7946>.

## Consequences

- Positive: a maintained parser with wide use; malformed input is an error value.
- Negative / trade-offs: one more dependency in the browser bundle (Q-5), measured by the size
  check.
- Follow-ups: none.

## Enforcement

A test of `adapter-geojson` reads every geometry type and a malformed document, and cites this ADR
with `Verifies:`.
