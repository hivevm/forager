# ADR-0009: The code is a Cargo workspace of one crate per responsibility, shaped as ports and adapters around a functional core, with the allowed dependency edges decided by a test

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `Cargo.toml`, every crate under `crates/`, the `[dependencies]` of each, and the dependency test of `crates/arch-test`

## Context

The specification asks for one core that runs in the browser and in a server (*Mission*,
*Vision*), free of I/O and C code (Q-1), deterministic (Q-2), knowing no MapServer syntax, with
all data behind async ports that require no `Send` (*Constraints*). Hosts differ — the CLI, a
browser page, a server, later wasmtime — and so do data sources: GeoJSON, FlatGeobuf, MVT, COG in
the client; Shapefile and GeoTIFF on the server, later PostGIS, OGR, GDAL. A missing adapter skips
its layer with a diagnostic (G-8). The brief proposes a workspace and its dependency direction.

The catalogue entry *Ports and adapters* fits when the domain outlives its technologies and must
be tested without them, and when agents work in parallel behind named ports. Its *Does not fit*
— a thin CRUD application, a small script — does not apply. *Functional core, imperative shell*
fits the core itself: symbolization, labelling, and rasterization are pure functions over values,
the effects live in hosts and adapters. In Rust, a crate is the unit whose dependencies the
compiler enforces: a module cannot use what its crate does not depend on.

## Decision

We will organise the code as one Cargo workspace with one crate per responsibility under
`crates/<name>`, published as `forager-<name>`, shaped as ports and adapters around a functional
core, and created only when the work first needs it:

| Group | Crates | May depend on |
|---|---|---|
| IR | `style-ir` | nothing of this workspace |
| Core | `expr`, `proj`, `symbolize`, `label`, `raster`, `render`, `render-agg`, `encode`, `engine` | `style-ir`; `expr`; `render` only on `style-ir`; `render-agg` on `render`; `engine` on any core crate and `ports` |
| Ports | `ports` | `style-ir`, `expr` |
| Frontend | `mapfile` | `style-ir`, `expr` |
| Adapters | `adapter-*` (GeoJSON first; Shapefile, GeoTIFF, FlatGeobuf, MVT, COG) | `ports`, `style-ir` |
| Hosts | `cli`, `bindings-wasm`, later `bindings-wit` | anything above |

- No core crate, the IR, the ports, or the frontend depends on an adapter or a host.
- No crate of the core, the IR, the ports, the frontend, or the browser bindings depends,
  directly or transitively, on a crate whose name ends in `-sys` (Q-1).
- The core does no I/O and is synchronous; async exists only in `ports`, the adapters, and the
  hosts, and no port trait requires `Send`.

**Out of scope:** the crates each responsibility uses from crates.io; the content of the ports;
the server host; the transformation nodes of `CONTOUR`, `KERNELDENSITY`, and their kind.

## Alternatives considered

- **One crate with modules** — fewer manifests, but the dependency direction becomes a convention
  no compiler enforces, and the core could not be built for `wasm32-unknown-unknown` without the
  server's adapters.
- **Layered** — the catalogue's familiar shape; the direction runs top-down, so the data sources
  would shape the core and its tests would need them.
- **All crates of the brief scaffolded at once** — about twenty empty crates that nothing tests;
  each is created by the first ticket that needs it.

## Sources / Prior art

- The catalogue entries *Ports and adapters* and *Functional core, imperative shell* of the
  `propose-adr` skill and their sources: Alistair Cockburn, *Hexagonal Architecture*
  (<https://alistair.cockburn.us/hexagonal-architecture/>).
- The brief's workspace structure and dependency direction, kept, with `cli` and
  `adapter-geojson` added (G-2, G-5).

## Consequences

- Positive: the core builds and is tested without any data source or host; agents work an adapter
  or a core crate without touching the others; the browser bundle contains only what
  `bindings-wasm` pulls in (Q-5).
- Negative / trade-offs: many small manifests; a change across the IR ripples through every crate
  that depends on it; the table above grows with each new crate and is changed by superseding this
  ADR where a new edge breaks it.
- Follow-ups: the ports' traits and the style IR's shape, each decided when the golden path first
  needs it.

## Enforcement

A test in `crates/arch-test` reads `cargo metadata` and fails on any workspace dependency edge
not allowed by the table above, and on any `-sys` crate in the dependency tree of the core, the
IR, the ports, the frontend, or the browser bindings; it cites this ADR with `Verifies:`.
`cargo deny` `bans` (ADR-0008) denies the known C-binding crates as a second net. Not checked:
that the core does no I/O beyond what its dependencies allow — the build of the core for
`wasm32-unknown-unknown`, which has no file system or network, catches most of it.
