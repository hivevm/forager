# Ticket: Render a mapfile's shapefile layers from their DATA

## What to build

`forager render` draws a mapfile whose layers read shapefiles, without `--source`. The frontend
reads `SHAPEPATH` and joins it with a `local` source's `DATA` into the source's `data`; the CLI
binds each layer of kind `local` to the shapefile adapter, inside the base context, beside the
GeoJSON files `--source` names, which win for their layer. The engine renders with sources of
both adapters.

## Acceptance criteria

- [ ] Given `SHAPEPATH "data"` and `DATA "poly"`, when the mapfile is rendered, then the layer
      draws `data/poly.shp` from the mapfile's directory.
- [ ] Given the suite's `renderers` directory and a mapfile there with `SHAPEPATH "../misc/data"`,
      when it is rendered with the suite as the base context, then the layer draws.
- [ ] Given a `DATA` or `SHAPEPATH` whose path leaves the base context, by `..` or as an
      absolute path, when the mapfile is loaded, then it is a diagnostic and no file outside the
      base context is read (Q-8).
- [ ] Given a broken shapefile, when the mapfile is rendered, then the layer is skipped with a
      diagnostic naming the layer, the source kind, and the source span (G-8).
- [ ] Given a `.shp` larger than the data-file limit, when it is read, then it is a diagnostic
      (Q-9).
- [ ] Given a layer named by `--source` and also of kind `local`, when the mapfile is rendered,
      then the GeoJSON file is drawn.

## Read first

- ADR-0007 (resource paths and limits stay with the maintainer), ADR-0009, ADR-0023 (the base
  context); Q-8, Q-9, G-8.
- Glossary: *source reference*, *base context*, *host*.
- Golden path: the CLI's GeoJSON binding and its render tests, which are the seam. It is blocked by
  revision ticket 04, which moves that binding into one function the hosts share.

## Blocked by

- docs/plan/conformance-harness/02-shapefile-adapter.md
- docs/plan/revision-2026-10-24/04-hosts-bind-sources-alike.md

## Mode

with the human: it changes the resolution of resource paths and applies a resource limit, which
the harness keeps with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
