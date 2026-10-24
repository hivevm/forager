# Ticket: The CLI renders a GeoJSON layer to a PNG

## What to build

`forager render <mapfile> --source <layer>=<file.geojson> -o <out.png>` binds the named layer to
GeoJSON features through the ports and the GeoJSON adapter, renders the map's extent at its size
with the layer's class styles, and writes a PNG. A layer without a bound source is skipped with a
diagnostic.

## Acceptance criteria

- [ ] Given a polygon layer and a GeoJSON polygon, when the map is rendered, then the RGBA buffer
      equals the committed reference image.
- [ ] Given a line layer and a GeoJSON line, when the map is rendered, then the RGBA buffer equals
      the committed reference image.
- [ ] Given a layer with no `--source`, when the map is rendered, then the layer is skipped and a
      diagnostic names the layer, the source kind, and the source span (G-8).
- [ ] Given an RGBA buffer, when it is encoded as PNG and decoded, then the buffer is identical
      (G-12).
- [ ] Given invalid GeoJSON, when the map is rendered, then a diagnostic reports it and the process
      does not panic (Q-3).
- [ ] The core crates touched build for `wasm32-unknown-unknown`, and the dependency test passes.

## Read first

- ADR-0009 (the edges of `ports`, `engine`, `render`, `render-agg`, `encode`, `adapter-geojson`);
  the golden path of `docs/ARCHITECTURE.md`; `docs/GLOSSARY.md` (port, adapter, source reference,
  display list, render mode).

## Blocked by

- docs/plan/browser-tracer/01-cli-reads-mapfile.md

## Mode

with the human — it defines the first port traits and chooses agg-rust, both kept with the human
by ADR-0007.
