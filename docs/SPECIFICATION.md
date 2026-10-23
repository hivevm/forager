# Specification — Forager

> The **specification** of this project: the problem it solves, where it is going, and what has to
> be true for it to have succeeded, written in the vocabulary of [`GLOSSARY.md`](GLOSSARY.md). It
> is the **constitution**: every Architecture Decision Record in [`adr/`](adr/) derives from it and
> must not contradict it ([ADR-0001](adr/0001-agent-governance-model.md)).

## Problem

Teams that maintain their cartography as UMN MapServer mapfiles can render it only on a server:
MapServer is a C program bound to CGI/FastCGI and to C libraries (GDAL, PROJ, AGG, FreeType), and
it does not run in a browser. To show the same maps client-side they render raster tiles on the
server, which costs server capacity and round-trips and leaves the client unable to restyle or
label per viewport, or they rewrite the styling by hand for a web renderer such as MapLibre or
OpenLayers, which loses MapServer semantics (classes, expressions, symbol scaling, the label
cache) and doubles maintenance. Nothing renders a mapfile with MapServer's semantics in
WebAssembly, and nothing embeds that semantics as a pure-Rust library in a server.

The evidence that this is worth solving now is not yet recorded; see *Assumptions*.

## Mission

Forager brings existing MapServer cartography to the browser without a map server, and keeps the
server path on the same code.

## Vision

Forager replaces MapServer's rendering path wherever pixels are produced: in the browser, in a
Rust server, and in a wasmtime sandbox. MapServer remains only as the semantic reference and the
test oracle.

## Strategy

Translate mapfiles at load time into a MapServer-free style IR whose expressions are CQL2. Keep
the core synchronous, free of I/O, and deterministic, with all data behind async ports and
adapters. Grow it in phases, each ending with a running WASM build and green conformance tests
against msautotest.

## Goals / Success Criteria

Acceptance criteria a test can decide, where it helps in the form *When <trigger>, the system
shall <response>*. Each starts with a bold `G-n`, which a test cites with `Verifies: G-n`. Until
that test exists the criterion is pending work, and the traceability check lists it
([ADR-0002](adr/0002-decisions-verified-by-tests.md)). Identifiers are never reused.
Only a normalization renumbers them ([`AGENTS.md` §3](../AGENTS.md#3-adr-rules), rule 5).

1. **G-1** — When a mapfile of the msautotest suite is loaded with a resource loader for its
   `INCLUDE`, `SYMBOLSET`, and `FONTSET` files, the system shall produce a style IR and no
   diagnostic of severity error; warnings are allowed.
2. **G-2** — When the CLI is given the path of a mapfile, it shall print the style IR as YAML or
   JSON and every diagnostic with severity, code, message, and source span, and exit non-zero if
   loading failed.
3. **G-3** — When a mapfile is converted mapfile → style IR → YAML → JSON → style IR, the system
   shall yield a style IR equal to the first.
4. **G-4** — When a style document of a newer minor format version is loaded, the system shall
   ignore its unknown fields with a diagnostic naming the field and its location; a newer major
   version shall be a load error, and an older major version shall be migrated to the current one.
5. **G-5** — When the WASM bindings receive a style, GeoJSON features, and a viewport, they shall
   return the rendered RGBA image, and the HTML example shall display it.
6. **G-6** — When a msautotest case is rendered, its RGBA buffer shall match the reference image
   within the tolerance of its category: geometry and symbols, raster nearest neighbour, raster
   bilinear or average, text.
7. **G-7** — When a filter is evaluated with and without pushdown, the system shall return
   identical feature sets.
8. **G-8** — When a layer's source has no bound adapter or its adapter fails, the system shall
   skip the layer and emit a diagnostic naming the layer, the source kind, and the source span.
9. **G-9** — When the same input is rendered natively and on `wasm32-unknown-unknown`, the system
   shall produce bit-identical RGBA buffers.
10. **G-10** — When labels are placed for a msautotest case, the system shall place and discard
    the same labels as the reference, each within 1 px of its reference position.
11. **G-11** — When a proj4 transformation used by a test case is evaluated, proj4rs shall agree
    with PROJ within a tolerance stated in metres.
12. **G-12** — When an RGBA buffer is encoded as PNG and decoded again, the system shall yield the
    identical buffer.

## Quality Goals

The few properties that drive decisions, in priority order. Every ADR weighs its *Consequences*
against them. Each has a measure concrete enough to settle a disagreement and starts with a bold
`Q-n`, cited like a goal. Security goals, what the system must never do, belong here, derived
from the threats below.

- **Q-1** — Portability: the core runs in the browser, so every core crate builds for
  `wasm32-unknown-unknown` and links no C code. Measured by CI building the core for that target,
  and by `cargo tree` of the core naming no `-sys` crate.
- **Q-2** — Determinism: a map must look the same wherever it is rendered, and reference images
  are only comparable if output is reproducible. The same input yields bit-identical RGBA buffers
  on every target (G-9); no output path depends on hash-map iteration order, a clock, or a random
  source.
- **Q-3** — Robustness: mapfiles and data come from users, and a panic in the browser loses the
  whole map. Invalid geometries, broken files, and errors of third-party crates become
  diagnostics. Measured by a fuzz run of 5 minutes per target in CI on every pull request — the
  mapfile parser, the YAML/JSON loader, and the renderer — that ends without a panic.
- **Q-4** — Fidelity: the point of Forager is MapServer's semantics. The share of in-scope
  msautotest cases that pass G-6 does not fall from one release to the next.
- **Q-5** — Bundle size: the core is downloaded by every browser that shows a map. The gzipped
  `.wasm` of the browser bindings stays under 3 MB; CI fails above it.
- **Q-6** — Licence hygiene: Forager is released as `MIT OR Apache-2.0` and must stay
  redistributable. Every dependency carries a permissive licence, measured by
  `cargo deny check licenses` being green.
- **Q-7** — Confidentiality: no credential reaches a client. The host rewrites source references
  before they leave the server, and the browser bindings reject a source reference with a
  credential property; a test feeds one and expects the rejection.
- **Q-8** — Containment: `INCLUDE`, `SYMBOLSET`, `FONTSET`, `SHAPEPATH`, and pixmap paths resolve
  only inside the base context the host gives. A path with a `..` escape or an absolute path is a
  diagnostic; a test feeds each and expects it.
- **Q-9** — Bounded resources: `INCLUDE` depth and cycles, image size, feature count, expression
  nesting, and regular-expression size are limited, and exceeding a limit is a diagnostic. A test
  per limit feeds an input past it and expects the diagnostic.

## Non-Goals

What this project explicitly does not try to do.

- Data queries: `TYPE QUERY`, `TEMPLATE`, `QUERYMAP`, hilite, GetFeatureInfo.
- CGI/FastCGI, MapScript, and OGC services (WMS, WFS, WCS).
- Runtime substitution of `%var%`, beyond the default from `VALIDATION` applied at load time.
- Vector output (SVG, PDF).
- An EPSG database in the core; `init=epsg:` is resolved only by a resolver the host injects.
- A strict mode that fails a map when one layer's source is unavailable.
- Pixel-identical output to MapServer: fidelity is a tolerance (Q-4), not equality.

## Constraints

What the solution has to live with regardless of the better choice: platforms, standards,
systems to interoperate with, limits of time, budget, and team. A constraint not written here is
rediscovered as a rejected alternative in an ADR.

- Core crates do no I/O — no file system, no network — link no C code, need no threads, and build
  for `wasm32-unknown-unknown`.
- Async exists only at the ports, and no async trait requires `Send`: the browser is
  single-threaded.
- Only the mapfile frontend knows MapServer syntax; the core sees the style IR alone.
- The core accepts projections only as proj4 strings.
- The licence is `MIT OR Apache-2.0`, and dependencies carry permissive licences only. Where logic
  is ported from MapServer's C code, the affected file keeps MapServer's copyright notice.
- `cargo fmt`, `cargo clippy -D warnings`, and all tests are green; identifiers and comments are
  in English.
- The reference is the MapServer source and its msautotest suite at the latest 8.x release,
  pinned by tag; moving the pin is a deliberate change with a full conformance run. Older mapfile
  syntax is read as far as that release accepts it.
- The team is one maintainer working with coding agents, without a fixed deadline or budget.

## Threats & Forbidden Actions

The assets, the actors who threaten them, and the trust boundaries between the parts. Then what
the system must never do, regardless of what a task asks. A prohibition not written here does not
exist for an agent. One a test can decide is restated as a `Q-n`, so that it is verified.

- **Assets:** credentials in connection strings (`CONNECTION`); the host's files behind the
  resource loader; the availability of the server and of the user's browser tab.
- **Actors and trust boundaries:** the authors of mapfiles and style documents and the providers
  of data files are untrusted, since a server may render maps its users upload. The boundary
  between host and core runs through the ports; the boundary between server and client runs
  through the source references the host hands out.
- The system shall never pass a connection string, or a source reference property that carries
  credentials, to a client (Q-7).
- The system shall never resolve a resource path outside the base context the host gives it
  (Q-8).
- The system shall never let one input exhaust memory or time without bound (Q-9).

## Assumptions

What is believed true today and would invalidate the decisions above if it turned out false:
expected scale, the shape of the data, who operates the result, which dependency stays
maintained.

- There is demand for rendering existing mapfiles in the browser; no evidence for it is recorded
  yet (see *Problem*).
- The pure-Rust crates the work leans on — agg-rust, proj4rs, rustybuzz, unicode-bidi, skrifa,
  i_overlay, shapefile, tiff, flatgeobuf — build for `wasm32-unknown-unknown` without C code and
  stay maintained. The ADR that picks each one verifies it.
- The msautotest reference images are reproducible enough that the tolerances of G-6 lie above
  their platform noise; the noise is measured before the first rendering criterion is held.
- proj4 strings cover the projections that client-side maps need.
- Browser hosts can serve HTTP range requests for COG and FlatGeobuf.
- The expected scale — features per viewport, image size, layer count — is not known yet; the
  limits of Q-9 are set from a first measurement.
