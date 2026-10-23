# Architecture — Forager

> The system **as it stands today**: the parts it is built from, what each is responsible for, and
> how they fit together. It holds no rule and no decision. It names the ADR behind each structural
> choice; where it disagrees with an accepted ADR, the ADR is right and this document is stale. It
> is updated in the same change as the structure it describes
> ([ADR-0001](adr/0001-agent-governance-model.md)).
>
> **Last design revision:** 2026-10-04, due after 20 changes. The revision that ran moves the
> date; the number is this project's to set; a sensor counts the changes outside `docs/` since
> the commit that set the date and says when the next is due
> ([ADR-0006](adr/0006-feature-layer.md)).

## Context

What sits outside the system and what crosses its boundary. A diagram earns its place here more
than anywhere else, and none beats one that has stopped being true.

A host embeds the core and owns all I/O: today the CLI, which reads files. The host hands the
mapfile frontend a resource loader rooted at the mapfile's directory, the base context; the
frontend returns a style document and diagnostics. To render, the host binds a feature source to
each layer by the layer's name and hands the engine a renderer; the engine returns an RGBA image
and diagnostics, and the host encodes it.

## Building blocks

The parts the system is made of, each with one responsibility, named in the vocabulary of
[`GLOSSARY.md`](GLOSSARY.md). One level deep, deeper only where the size of a part earns it. The
golden path and the test pattern an agent copies from are named here.

- **`style-ir`** — the data model of a map, the diagnostics with their source spans, and the
  style document that serializes it as YAML or JSON through serde
  ([ADR-0014](adr/0014-style-document-serde.md)). It depends on no other crate of the workspace.
- **`ports`** — the port traits, today the resource loader, and the resource path that cannot
  leave the base context (Q-8). Async lives here and in hosts and adapters, never `Send`.
- **`mapfile`** — the mapfile frontend: a lexer that follows MapServer's, a generic parser of
  blocks and directives driven by the statement keywords of each block kind, `INCLUDE` splicing
  bounded in depth, cycles, and total size (Q-9), and the translation into the style IR, which
  reports every keyword it does not carry over. It is the only crate that knows MapServer syntax.
- **`engine`** — renders a map: fits its extent to the image as MapServer does, asks each layer's
  bound feature source for the features in view, and turns each feature and style into display
  items by the layer's render mode. A layer without a source, or whose source fails, is skipped
  with a diagnostic (G-8). It is the one core crate that awaits the ports.
- **`render`** — the display list, the renderer trait, and the RGBA image.
- **`render-agg`** — the reference renderer on agg-rust; it clips every path near the image
  before rasterizing, and is the one crate that may use `unsafe`
  ([ADR-0015](adr/0015-agg-rust-rasterizer.md)).
- **`encode`** — PNG from and to an RGBA image ([ADR-0017](adr/0017-png-crate.md)).
- **`adapter-geojson`** — a feature source over a GeoJSON document
  ([ADR-0016](adr/0016-geojson-crate.md)).
- **`cli`** — the `forager` command, a host with a file loader that also refuses symbolic links
  out of the base context; `forager render` binds GeoJSON files to layers and writes a PNG.
- **`arch-test`** — the dependency test of [ADR-0009](adr/0009-ports-and-adapters-crates.md):
  the allowed edges between the crates, and no C code under the IR, the core, the ports, or the
  frontend.

The crates and their allowed edges are [ADR-0009](adr/0009-ports-and-adapters-crates.md); the
toolchain and its checks are [ADR-0008](adr/0008-rust-toolchain.md).

**Golden path:** `forager read`, from the CLI's argument through the file loader and the
frontend's `load` to the style document — `crates/cli/src/main.rs`, `crates/mapfile/src/lib.rs`.
**Test pattern:** behaviour is tested at the highest seam that decides it — the frontend's `load`
with files in memory (`crates/mapfile/src/tests.rs`), the binary as a process with fixtures
(`crates/cli/tests/read.rs`) — each test naming the goal or ADR it verifies with `Verifies:`.

## How it runs

The few paths worth following end to end, such as a request, a job, or a build, and where state
lives between them. Only what a newcomer would otherwise reconstruct from code.

`forager read <mapfile>`: the CLI canonicalizes the mapfile's path and roots a file loader at its
directory. The frontend reads the mapfile through the loader, lexes it, and splices in each
`INCLUDE`'s tokens, resolved against that directory as MapServer resolves them. The parser builds
the `MAP` block; the translation walks it into the style IR and adds a warning for each keyword
not carried over. With no error, the CLI prints the style document; diagnostics go to standard
error. No state outlives the call.

`forager render <mapfile> --source <layer>=<file> -o <png>` loads the mapfile the same way, parses
each GeoJSON file into a source bound to its layer, and runs the engine with the agg-rust renderer.
For each layer that is ON or DEFAULT, in order, the engine queries the source with the fitted
extent and draws every feature in each style of the first class; the image is encoded as PNG.
