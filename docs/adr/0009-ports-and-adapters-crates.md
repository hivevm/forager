# ADR-0009: The code is a Cargo workspace of one crate per responsibility, shaped as ports and adapters around a functional core, with the allowed dependency edges decided by a test

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `Cargo.toml`, every crate under `crates/`, the `[dependencies]` of each, and the dependency test of `crates/arch-test`

## Context

The specification asks for one core that runs in the browser and in a server (*Mission*,
*Vision*), free of I/O and C code (Q-1), deterministic (Q-2), knowing no MapServer syntax, with
all data behind async ports that require no `Send` (*Constraints*). Hosts differ — the CLI (G-2),
a browser page through the bindings (G-5), a server, later wasmtime — and so do data sources:
GeoJSON, FlatGeobuf, MVT, and COG in the client; Shapefile and GeoTIFF on the server. A missing
adapter skips its layer with a diagnostic (G-8). The style IR's expressions are CQL2 (*Strategy*),
and the part of a filter an adapter can evaluate is pushed down to it (G-7); so the frontend,
the core, the ports, and the adapters all need the CQL2 syntax tree.

The catalogue entry *Ports and adapters* fits when the domain outlives its technologies and must
be tested without them, and when agents work in parallel behind named ports. Its *Does not fit*
— a thin CRUD application, a small script — does not apply. *Functional core, imperative shell*
fits the core itself: symbolization, labelling, and rasterization are pure functions over values,
the effects live in hosts and adapters. In Rust a crate is the unit whose dependencies the
compiler enforces: code cannot use what its crate does not depend on.

## Decision

We will organise the code as one Cargo workspace with one crate per responsibility under
`crates/<name>`, with the package name `forager-<name>`, shaped as ports and adapters around a
functional core, each crate created only when the work first needs it, and with exactly these
dependency edges between workspace crates:

| Group | Crate | May depend on |
|---|---|---|
| IR | `style-ir` — the data model, diagnostics, the style document format, and the CQL2 syntax tree | — |
| Core | `expr` — evaluation of CQL2 and the pushdown planner | `style-ir` |
| Core | `proj` | `style-ir` |
| Core | `render` — the display list and the renderer trait | `style-ir` |
| Core | `render-agg` | `style-ir`, `render` |
| Core | `encode` | `render` |
| Core | `symbolize`, `label`, `raster` | `style-ir`, `expr`, `proj`, `render` |
| Core | `engine` — orchestration of a request | every core crate, `ports` |
| Ports | `ports` — the port traits | `style-ir` |
| Frontend | `mapfile` | `style-ir`, `ports` |
| Adapters | `adapter-<kind>` | `style-ir`, `ports` |
| Hosts | `cli`, `bindings-wasm`, `dev-server` (serves the HTML example), later `bindings-wit` | every crate above |
| Tooling | `arch-test` | — |

- Every core crate but `engine` is synchronous and does no I/O. `engine` is the one core crate
  that awaits the ports. The mapfile frontend awaits the resource loader too, since the files an
  `INCLUDE` names are known only once the including file is parsed. Async otherwise exists only
  in `ports`, the adapters, and the hosts, and no port trait requires `Send`.
- No crate of the IR, the core, the ports, or the frontend depends, directly or transitively, on
  a crate that links C code; a crate whose name ends in `-sys` is taken to link C code, except the
  pure-Rust JavaScript bindings `js-sys` and `web-sys`, which only hosts and adapters may use.

**Out of scope:** the crates each responsibility uses from crates.io; the content of the ports and
of the style IR; the server host; transformation layers such as contours and their crate.

## Alternatives considered

- **A synchronous frontend handed every file up front** — the host would have to parse to learn
  the `INCLUDE`s, or load in rounds, one per level of nesting; awaiting the loader is simpler.
- **One crate with modules** — fewer manifests, but the dependency direction becomes a convention
  no compiler enforces, and the core could not be built for `wasm32-unknown-unknown` without the
  server's adapters.
- **The CQL2 syntax tree in `expr`** — then `style-ir` would depend on `expr`, or the IR would
  hold expressions in another form; the tree is data the style document serializes, so it belongs
  to the IR, and adapters read it for pushdown without depending on the evaluator.
- **The CQL2 syntax tree in a crate of its own** — one more crate every other one depends on, for
  no boundary the IR does not already draw.
- **Layered** — the catalogue's familiar shape; the direction runs top-down, so the data sources
  would shape the core and its tests would need them.
- **All crates scaffolded at once** — about twenty empty crates that nothing tests; each is
  created by the first ticket that needs it.

## Sources / Prior art

- The catalogue entries *Ports and adapters* and *Functional core, imperative shell* of the
  `propose-adr` skill and their sources: Alistair Cockburn, *Hexagonal Architecture*
  (<https://alistair.cockburn.us/hexagonal-architecture/>).
- OGC Common Query Language (CQL2), <https://docs.ogc.org/is/21-065r2/21-065r2.html>.

## Consequences

- Positive: the core builds and is tested without any data source or host; agents work an adapter
  or a core crate without touching the others; the browser bundle contains only what
  `bindings-wasm` pulls in (Q-5).
- Negative / trade-offs: many small manifests; a change of the IR ripples through every crate. An
  edge the table does not allow is added by superseding this ADR, which makes the table a
  deliberate bottleneck.
- Follow-ups: the shape of the port traits and of the style IR, each decided when the golden path
  first needs it.

## Enforcement

A test in `crates/arch-test` reads `cargo metadata` and fails on any dependency edge between
workspace crates that the table does not allow, on a workspace crate the table does not name, and
on any `-sys` crate, `js-sys` and `web-sys` included, in the dependency tree of the IR, the core,
the ports, or the frontend; it cites this ADR with `Verifies:`. The build of the browser crates
for `wasm32-unknown-unknown` ([ADR-0008](0008-rust-toolchain.md)), which has no file system or
network, catches most I/O in the core; that only `engine` and the frontend await is review.
