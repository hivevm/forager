# Ticket: Carry the layer's projection and the scale ranges of maps, layers, and classes

## What to build

A layer's `PROJECTION` loads into the IR as a proj4 string, resolved as the map's is
(ADR-0018), with `AUTO` kept as "the map's projection". The map's `UNITS` and the scale ranges
load: `MINSCALEDENOM`/`MAXSCALEDENOM` on layers, classes, labels, and styles, the old
`MINSCALE`/`MAXSCALE` spellings, and the layer's `SYMBOLSCALEDENOM`, which with a style's
`MINSIZE`/`MAXSIZE` becomes the scale factor of the spec's *Units and scale*. Writes the
`docs/semantics/` pages of these parameters, the formula of `SYMBOLSCALEDENOM` with its
MapServer source among them.

## Acceptance criteria

- [ ] Given a layer with `PROJECTION "init=epsg:4326" END`, when it loads with a resolver, then
  the layer's projection is the proj4 string; without one, it is the load error of ADR-0018.
- [ ] Given `MINSCALEDENOM 1000` and `MAXSCALEDENOM 50000` on a layer and a class, and
  `MAXSCALE 50000`, when they load, then the IR holds each range, and the old spelling the same
  as the new.
- [ ] Given `SYMBOLSCALEDENOM 25000` and a style `SIZE 10 MINSIZE 4 MAXSIZE 20`, when it loads,
  then the IR holds what the engine needs to compute MapServer's size at any scale, by the
  formula on the semantics page.
- [ ] Given `UNITS meters`, when it loads, then the map's units are in the IR.
- [ ] Given each of these, when the document round-trips through YAML and JSON, then it is equal.

## Read first

- [ADR-0018](../../adr/0018-projection-resolver.md),
  [ADR-0014](../../adr/0014-style-document-serde.md);
  the spec next to this ticket (*Units and scale*, *Projection*).
- MapServer `rel-8-6-6`: `loadLayer` and `loadClass` in `src/mapfile.c`, the scale factor in
  `src/mapdraw.c` (`msDrawVectorLayer`, `layer->scalefactor`).
- The map-level `PROJECTION` translation in the frontend and its tests.

## Blocked by

- docs/plan/ir-completeness/07-styles-complete.md

## Mode

with the human: it changes the style document format ([ADR-0007](../../adr/0007-harness.md)).
