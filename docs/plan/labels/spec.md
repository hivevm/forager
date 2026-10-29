# Spec: Point labels

## Problem

A map without its labels is not the map its mapfile describes. Forager reads every `LABEL` of a
mapfile into the style IR, but the engine draws none, so every msautotest case with a label is out
of the harness's scope and a user's map loses its place names. Nothing measures where Forager would
place a label, nor which labels it would leave out where they collide, which is what G-10 asks.

## Solution

The maintainer and a user see the labels of point features drawn as MapServer draws them: the text
of each label, in the font its `FONTSET` names, shaped as MapServer shapes it, at the position and
offset the label asks for, in its colour with its outline, and left out where it would collide
with a label already placed, by MapServer's label cache and its priorities. The harness measures
each case of the category text against its reference image, and each label's position and whether
it was placed against MapServer's own, so that a label in the wrong place is named, not only a
patch of differing pixels.

This spec covers labels of point features with TrueType fonts and Latin script. Labels along lines
and in polygons, right-to-left and complex scripts, and bitmap fonts are later specs on the same
text path.

## Existing behaviour

- The frontend reads `LABEL`, `FONTSET`, and the label properties into the style IR, with the
  bindings and expressions MapServer allows; its tests pin the translation. Three tickets of the
  revision of 2026-10-24 still correct it: a label's bound properties as MapServer binds them, a
  label's `EXPRESSION`, which is ignored today, and `TEXT` as MapServer evaluates it.
- The font set is read through the resource loader against the mapfile's directory; the font
  files it names are not read.
- The engine draws points, lines, and polygons by each layer's render mode, and nothing of a label.
  Its tests pin that with Forager's own reference images; they stay as they are.
- The conformance harness lists every case with a label out of scope with the reason *labels*.
  The noise measurement covers the category geometry and symbols only; G-13, the categories raster
  and text, has no tolerance yet.
- The browser bindings take a mapfile, GeoJSON per layer, an EPSG table, and a viewport; nothing
  carries a font.

## Goals served

- G-10, for labels of point features.
- G-13, for the category text.
- Q-4: the share of passing cases grows by the label cases brought into scope.
- Q-8 and Q-9 are touched: font files resolve inside the base context, and their size is bounded.
- G-9 and Q-2: the text path renders the same bytes natively and in the browser, with no hash map,
  clock, or random source in the core.

## User stories

1. As a user, I want the labels of my point layers drawn, so that the rendered map shows the names
   my mapfile asks for.
2. As a user, I want a label's text taken from its `TEXT`, its class, or the layer's `LABELITEM`,
   so that each feature shows its own name.
3. As a user, I want the font my `FONTSET` names used, with its kerning, so that the text looks as
   it does in MapServer.
4. As a user, I want `POSITION`, `OFFSET`, `ANGLE`, `ALIGN`, `WRAP`, `COLOR`, `OUTLINECOLOR`, and
   `OUTLINEWIDTH` honoured, so that a label sits and looks where MapServer puts it.
5. As a user, I want labels that would overlap left out by `PRIORITY` and `BUFFER`, with `FORCE`
   and `PARTIALS` as MapServer reads them, so that the map stays readable and matches MapServer's.
6. As a browser host author, I want to hand Forager the font files the font set names, as I hand it
   GeoJSON, so that the bundle stays small and the fonts are the page's choice.
7. As the maintainer, I want the label cases of the category text compared with their references,
   so that I see which ones Forager draws as MapServer does.
8. As the maintainer, I want each label's position and placed or discarded status compared with
   MapServer's own, so that a wrong label is named with how far off it is.
9. As the maintainer, I want the tolerance of the category text set above MapServer's own noise, so
   that a failure is Forager's and not the platform's.

## Implementation decisions

- **Text stack.** Shaping by harfrust, glyph outlines and metrics by skrifa, rasterized as paths
  by the reference renderer, as MapServer's AGG renderer draws FreeType's outlines. Whether hinting
  is applied, and how, follows what MapServer does at the pin. The crates and their versions are
  an ADR (the specification's Assumption asks one per crate); it weighs them for
  `wasm32-unknown-unknown`, licence, and maintenance.
- **Fonts through the loader.** A host hands font files through the resource loader, as it hands
  every other file the mapfile names; the font set maps a font name to a path inside the base
  context. The CLI reads them from disk; the browser bindings take them from the page, by the
  path the font set names. A font that does not load leaves its labels undrawn with a diagnostic
  naming the font and the label (G-8 in spirit), never a substitute font. A font file is bounded in
  size (Q-9). Whether this needs a port of its own or the resource loader serves is decided by the
  ADR of the text stack.
- **Placement.** The engine places a label at its point feature, by `POSITION` and `OFFSET` in
  pixels, `ANGLE`, and `ALIGN` and `WRAP` for several lines, sized by `SIZE` in points scaled as
  MapServer scales it; a label's `STYLE`s, drawn with it, follow later.
- **Label cache.** Labels are collected over all layers and placed after them, by `PRIORITY`, then
  in MapServer's order; a label whose box, grown by `BUFFER`, meets one already placed or leaves
  the image is discarded unless `FORCE` or `PARTIALS` says otherwise. The order, the boxes, and the
  tests of collision follow MapServer's label cache at the pin.
- **Positions from MapServer.** The MapServer that the noise measurement builds also writes, per
  case, each label's text, position, and whether it was placed. How it does so, unmodified or with
  an instrumentation kept in the repository, is an ADR that revises ADR-0027.
- **Scope.** The cases with labels of point features only, TrueType fonts, and Latin script come
  into scope under the category text, each listed as passing or failing; every other label case
  keeps its reason, named more precisely (labels on lines, labels in polygons, a bitmap font,
  complex script).

## Testing decisions

- The engine's text path is tested at the engine with Forager's own reference images, as its
  geometry tests are: one label, each position, an outline, a collision.
- Placement and the label cache are tested on their own, as pure functions of label boxes, without
  a font.
- The harness compares the category text by the metric, and the positions and statuses label by
  label against MapServer's file, in the conformance test beside the geometry cases.
- The noise measurement covers the category text as it covers geometry and symbols.
- The font loading of the CLI and the bindings is tested at each host, the bindings with a font
  passed from a test as a page would pass it.

## Out of scope

- Labels along lines (`ANGLE FOLLOW`, `AUTO`, repetition) and in polygons, and leader lines.
- Right-to-left and complex scripts (FriBiDi's part); the shaping engine is in from the start.
- Bitmap fonts (`TYPE BITMAP`), and `ENCODING` other than UTF-8.
- A label's `STYLE`s, shadows, and `MAXOVERLAPANGLE`.
- A font bundled with Forager.
- The legend and the scale bar, which draw text of their own.

## ADR impact

- Binding: ADR-0009 (where the text stack lives among the crates), ADR-0012 (every new crate),
  ADR-0015 (the reference renderer draws the glyphs), ADR-0023 (the base context the fonts resolve
  in), ADR-0027 (the MapServer build the positions come from), ADR-0007 (the specification and the
  resource limits stay with the maintainer).
- New `proposed` ADRs before the tickets that need them: the text stack (shaping, glyphs, hinting,
  and how fonts reach the engine); the label positions out of MapServer, revising ADR-0027.
- A specification change: the threshold and the tolerance of the category text in G-13, set from
  the noise measurement.
