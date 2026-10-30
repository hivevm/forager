# Ticket: The label cache keeps labels the map's edge buffer away from the image's edge

## What to build

A map whose `WEB` `METADATA` sets `labelcache_map_edge_buffer` keeps every label that is not
partial that many pixels inside the image, as MapServer's `msDrawLabelCache` reads it at the pin:
the absolute value of `atoi` of the text becomes the label cache's gutter, and `labelInImage`
requires a box to lie at least the gutter inside each edge, its turned corners where its box does
not. Today the style IR does not carry the map's metadata, so the gutter is always 0, and a label
near the edge that MapServer leaves out is drawn.

The style IR carries the map's edge buffer; the engine hands it to the label cache.

## Acceptance criteria

- [ ] Given a mapfile whose `WEB` `METADATA` sets `labelcache_map_edge_buffer` to `"10"`, or to
      `"-10"`, when it is loaded, then the IR carries a gutter of 10; given text `atoi` reads as
      0, or none, then it carries none.
- [ ] Given a gutter of 10 and a label whose box lies 5 pixels inside the image's left edge, when
      rendered, then it is left out; given `PARTIALS TRUE` or `FORCE ON`, then it is drawn.
- [ ] Given a gutter and a turned label whose box crosses it but whose corners do not, then it
      is drawn as `labelInImage` decides.
- [ ] Given a style document of an earlier version, then it loads as before (ADR-0020).

## Read first

- ADR-0007 (the style document format stays with the maintainer), ADR-0020, ADR-0028.
- MapServer at the pin: `msDrawLabelCache`, src/mapdraw.c (the metadata read), `labelInImage`
  and `msTestLabelCacheCollisions`, src/maplabel.c.
- The label cache of the label crate and its tests of the image's edge.
- The msautotest case that sets it: renderers/multilabel-leader.map.

## Blocked by

- None (can start immediately)

## Mode

with the human: the style IR must carry the map's edge buffer, which changes the style document
format that ADR-0007 keeps with the maintainer, and how much of `WEB` `METADATA` the IR carries is
a decision of its own.
