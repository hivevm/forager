# Spec: Browser tracer — from a mapfile on the command line to a map in the browser

## Problem

A cartographer with an existing mapfile has no way yet to see what Forager makes of it: nothing
reads a mapfile, nothing renders, and nothing runs in a browser. Every later phase needs this path
to exist before it can be deepened.

## Solution

The cartographer runs the CLI on a current MapServer 8.x mapfile and sees the style IR it was
translated into, as YAML or JSON, with every diagnostic and its source span. They render a layer
of that mapfile from a GeoJSON file to a PNG on the command line. And they open an HTML page,
paste a mapfile and GeoJSON, and see the map drawn in the browser by the WebAssembly build of the
same core.

## Existing behaviour

Greenfield, nothing exists yet.

## Goals served

- G-2 — the CLI reads a mapfile and prints the style IR and diagnostics.
- G-5 — the browser bindings render GeoJSON features into an RGBA image the HTML example shows.
- G-8 — a layer without a bound source is skipped with a diagnostic.
- G-12 — the PNG encoder round-trips the RGBA buffer.
- Q-1, Q-5 — the core and the bindings build for `wasm32-unknown-unknown`; the bundle is
  measured.
- Q-8 — `INCLUDE` resolves only inside the base context.

## User stories

1. As a cartographer, I want to run the CLI on my mapfile, so that I see whether Forager reads it
   and what it understood.
2. As a cartographer, I want every keyword Forager does not handle yet reported with its line, so
   that I know what the rendering will lack.
3. As a cartographer, I want an `INCLUDE` resolved relative to the mapfile, and refused outside
   its directory, so that shared fragments work and nothing else is read.
4. As a cartographer, I want to choose YAML or JSON output, so that I can read it or feed it to
   tools.
5. As a cartographer, I want to render a layer from a GeoJSON file to a PNG, so that I see the
   styling applied without a browser.
6. As a web developer, I want to call one function of the WebAssembly module with a mapfile,
   GeoJSON, and a viewport, so that I get pixels to put on a canvas.
7. As a web developer, I want an example page and a one-command local server, so that I can try
   it without any other tooling.

## Implementation decisions

- The mapfile frontend reads the syntax generically — blocks closed by `END`, keywords with their
  values, strings, numbers, comments, `INCLUDE` — so that every syntactically valid mapfile
  parses. A first set of keywords maps into the style IR: the map's name, extent, size, image
  colour, and projection string; a layer's name, render mode, status, and source reference; a
  class's name; a style's colour, outline colour, and width. Every other known keyword is a
  warning "not yet supported"; keywords out of scope (`TEMPLATE`, `QUERYMAP`, and the like) are a
  warning "discarded"; unknown keywords are a warning too, since the frontend cannot yet know them
  all.
- Diagnostics carry severity, code, message, and source span from the first ticket on; the style
  IR carries source spans of its elements.
- The style document carries `version: "1.0"`; versioning beyond writing and checking that field
  is the next spec's.
- A layer's source reference keeps `CONNECTIONTYPE`, `CONNECTION`, and `DATA` unparsed. The CLI
  and the browser bindings bind a layer to GeoJSON by the layer's name; in the bindings, GeoJSON
  is passed in memory.
- Rendering in the tracer draws polygons filled and outlined, lines stroked, and points as
  squares of the style's size, in map units mapped linearly onto the image extent; no projection
  is applied.
- The rasterizer is agg-rust behind the renderer trait; the encoder writes PNG.

## Testing decisions

- The mapfile frontend is tested at its public function, from mapfile text and a resource
  loader in memory to style IR and diagnostics.
- The CLI is tested end to end as a process: mapfile fixture in, YAML/JSON and exit code out.
- Rendering is tested at the engine: style IR and GeoJSON in, RGBA buffer out, compared to a
  reference buffer committed as PNG; the comparison is exact, since it compares Forager with
  itself.
- The browser bindings are tested by building them for `wasm32-unknown-unknown` and by a native
  test of the function behind the binding; a test inside a browser is a later spec's.

## Out of scope

- The full mapfile: MapServer expressions and CQL2, symbols, labels, `SYMBOLSET`, `FONTSET`,
  `%var%`, `init=epsg:` resolution, and loading all msautotest mapfiles (G-1, G-3) — the next
  spec.
- Projection, raster, labels, and conformance against MapServer reference images (G-6).
- Tiles; a viewport is one image.

## ADR impact

Binding: ADR-0007 (harness), ADR-0008 (toolchain), ADR-0009 (crates and edges), ADR-0010 (Dev
Container), ADR-0012 (crate supply chain). The first ticket is the golden path that ADR-0008 and
ADR-0009 flip to accepted with. Each crate taken from crates.io is chosen in the ticket that first
needs it; agg-rust, the YAML serializer, and the GeoJSON parser are each an ADR before that
ticket's implementation, since they are costly to reverse.
