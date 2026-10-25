# Ticket: Render a mapfile's shapefile layers from their DATA

## What to build

`forager render` draws a mapfile whose layers read shapefiles, without `--source`: the frontend
reads `SHAPEPATH`, a layer of kind `local` resolves its `DATA` against it inside the base context,
and a new shapefile adapter reads the `.shp`, `.shx`, and `.dbf` through the resource loader.
`--source` keeps working and wins for the layer it names. The crate the adapter uses is decided
first, through `propose-adr`.

## Acceptance criteria

- [ ] Given a `proposed` ADR choosing the shapefile crate, when the maintainer accepts it, then
      the adapter is built on that crate and the ADR's test cites it.
- [ ] Given fixture shapefiles of points, lines, polygons with a hole, multipart shapes, and a
      null shape, when the adapter is queried, then it returns those features with their
      attributes, the null shape skipped.
- [ ] Given a truncated `.shp`, a `.shx` that disagrees with it, or a missing `.dbf`, when the
      layer is rendered, then the layer is skipped with a diagnostic naming the layer, the source
      kind, and the source span (G-8), and nothing panics (Q-3).
- [ ] Given `SHAPEPATH "data"` and `DATA "poly"`, when the mapfile is rendered, then the layer
      draws `data/poly.shp` from the mapfile's directory.
- [ ] Given a `DATA` or `SHAPEPATH` with a `..` escape or an absolute path, when the mapfile is
      loaded, then it is a diagnostic and no file outside the base context is read (Q-8).
- [ ] Given a `.shp` larger than the data-file limit, when it is read, then it is a diagnostic
      (Q-9).

## Read first

- ADR-0009 (an `adapter-<kind>` crate and its edges), ADR-0012 (the new crate), ADR-0016 (the
  shape of a crate-choice ADR), ADR-0023 (the base context), ADR-0007 (resource paths stay with
  the maintainer).
- Glossary: *source reference*, *base context*, *resource loader*, *adapter*.
- Golden path: the GeoJSON adapter and its binding in the CLI; the CLI's render tests are the seam.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides a crate by ADR and changes the resolution of resource paths, which the
harness keeps with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
