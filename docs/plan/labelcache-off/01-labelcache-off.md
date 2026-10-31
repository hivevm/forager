# Ticket: A layer with LABELCACHE OFF draws its labels as it draws its features

## What to build

A point layer whose mapfile sets `LABELCACHE OFF` draws each feature's labels at once, as
MapServer's `pointLayerDrawShape` does at the pin: for each label whose `msGetLabelStatus` is on,
`msDrawLabel` draws its text at the point, by its position and offset, with no test against other
labels, the markers, or the image's edge, in the layer's order of drawing, under the features of
later layers. Today the mapfile frontend warns that `LABELCACHE` is not supported and ignores it,
so such a layer's labels go through the label cache: they may be left out where they collide or
leave the image, and are drawn after every layer.

The style IR carries the layer's `LABELCACHE`; the engine draws a layer's labels after its features
when it is off, and adds them to the label cache otherwise.

## Acceptance criteria

- [ ] Given a mapfile with a layer's `LABELCACHE OFF`, when it is loaded, then the IR carries it
      and no warning is given; given `ON` or none, then the IR carries the label cache as today.
- [ ] Given two overlapping labels of a layer with `LABELCACHE OFF`, when rendered, then both are
      drawn, in the order of their features, and a label past the image's edge is drawn too.
- [ ] Given such a layer before a layer of polygons that cover its points, when rendered, then
      the polygons cover its labels.
- [ ] Given such a layer beside one whose labels are cached, then the cached labels are not tested
      against the labels drawn at once, as `msTestLabelCacheCollisions` knows only the cache.
- [ ] Given such a layer, when rendered, then `Rendered.labels`, which reports the label cache's
      members for the comparison of G-10, does not list its labels; the G-10 comparison of a case
      with such a layer says that those labels are not compared, as the reader of MapServer's
      cache (`crates/cli/tests/conformance/mapserver_labels.c`) does not see them either.
- [ ] Given a style document of an earlier version, then it loads as before (ADR-0020).

## Read first

- ADR-0007 (the style document format stays with the maintainer), ADR-0020, ADR-0028.
- The glossary's *Label cache*.
- MapServer at the pin: `pointLayerDrawShape` and `msDrawLabel`, src/mapdraw.c; `loadLayer`,
  src/mapfile.c (`LABELCACHE`); `msGetLabelStatus`, src/maputil.c.
- The label cache of the label crate and the engine's labels of point layers, and their tests.
- msautotest cases that set it: renderers/labelcacheoff.map, labelpnt.map, labelrot.map,
  utfgrid_truetype_symbol.map, misc/autostyle_quoted.map.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a layer property to the style document, whose format ADR-0007 keeps with
the maintainer.
