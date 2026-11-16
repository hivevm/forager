# Spec: Fidelity of the geometry and symbols cases, and a test for every criterion

## Problem

A cartographer who brings a mapfile to Forager expects it drawn as MapServer draws it. Of the
msautotest cases of misc and renderers that the conformance list holds in scope for geometry and
symbols, 25 are still listed as failing. Some fail for what Forager does not draw yet: TrueType
symbols, vector symbols as a polygon fill, circle layers, a style's `ANTIALIAS FALSE`, a style's
`GEOMTRANSFORM` expressions, `buffer` and `simplify`, and the map's `ANGLE`. Others fail for what
the specification already leaves out, or what the maintainer has decided to leave out, yet the list
counts them in scope, so the share Q-4 measures is held down by cases no work will ever make pass.

The maintainer, reading the record, also cannot trust that every criterion is met: G-7 asks for
pushdown, which no source has, and G-14, the browser example, has no test. Seven diagnostic codes
are written by the core and named by no test, so a change can silently stop emitting them.

## Solution

A mapfile that uses TrueType or vector symbols, circle layers, aliased lines, the vertex and shape
transforms of a style, `buffer` and `simplify`, or a rotated map is drawn as MapServer draws it,
natively and in the browser alike. The conformance list holds in scope only what the specification
asks for, and a case left out names the non-goal that leaves it out. Every criterion of the
specification is cited by a test: G-7 leaves the specification until a source needs pushdown, and
G-14 is held by a test that opens the browser example. Every diagnostic code the core writes is
named by a test that pushes it through a load or a render.

## Existing behaviour

- The conformance harness renders each case in scope, compares it by the metric of its category, and
  holds the outcome the list gives it; the comparison of G-9 renders the same cases through the
  browser bindings' wasm32 build. Every case listed as passing must stay passing; that is the
  regression suite of this work. Each ticket flips its cases from failing to passing.
- The list's header names the reasons for leaving a case out as the categories' exclusions. The
  harness reads a reason as free text and refuses only an empty one, so adding a kind of reason
  changes the header's prose, not the harness. The ten cases of `SCALETOKEN` give a runtime
  substitution, a reason the header already names.
- The mapfile frontend reads TrueType and SVG symbols into the style IR, with their glyph, font, or
  image; the symbolizer draws neither, and the marker size of either is reported as not computed.
- A circle layer is read as a polygon layer. A style's `ANTIALIAS` and the map's `ANGLE` are not in
  the style IR.
- A `GEOMTRANSFORM` expression is a transform tree (ADR-0048). A layer's tree is drawn. On a line or
  polygon layer, a class's style with a drawn tree is reported and draws nothing; on a point layer
  the style draws at the layer's points. `buffer`, `simplify`, and `simplifypt` are
  `not_drawn` nodes.
- The specification's G-7 is cited by no test; the traceability check lists it with G-14 as pending.

## Goals served

- Q-4 (fidelity): the share of in-scope cases that pass rises, by cases made to pass and by cases
  out of scope that never belonged in it.
- G-6, G-9: each case flipped holds the metric of G-6 natively and is bit-identical on wasm32.
- ADR-0002's traceability: every criterion cited by a test (G-7 removed, G-14 tested).
- G-14, G-5: the browser example shows the bindings' image and their diagnostics, under test.
- Q-3 (robustness): each diagnostic the core writes is pushed by a test, so a regression that drops
  one is caught.
- Q-1, Q-5, Q-6: the geometry library for `buffer` and `simplify` builds for wasm32 without C, stays
  inside the bundle budget, and carries a permissive licence.

## User stories

1. As a cartographer, I want a TrueType symbol drawn at a point, along a line, and as a polygon fill
   as MapServer draws it, so that icon fonts in my mapfile work in the browser.
2. As a cartographer, I want a vector symbol tiled as a polygon's fill, so that arrow and hatch-like
   fills keep their look.
3. As a cartographer, I want a circle layer drawn from its two corner points, so that its circles
   appear where MapServer puts them.
4. As a cartographer, I want `ANTIALIAS FALSE` on a style to draw aliased lines, so that crisp
   one-pixel lines stay crisp.
5. As a cartographer, I want a style's `GEOMTRANSFORM` expressions, `vertices`, `start`, and `end`
   with `ANGLE AUTO`, drawn, so that arrowheads and vertex marks appear.
6. As a cartographer, I want `buffer`, `simplify`, and `simplifypt` drawn as MapServer's GEOS draws
   them, so that offset outlines and generalized lines match.
7. As a cartographer, I want a map with an `ANGLE` drawn rotated, its symbols turned with it, so
   that north-up is not forced on rotated maps.
8. As the maintainer, I want the conformance list to leave out what the specification leaves out,
   each case naming its non-goal, so that Q-4 measures only what Forager promises.
9. As the maintainer, I want every criterion cited by a test, so that the traceability check lists
   no pending criterion and a regression of G-14 fails CI.
10. As the maintainer, I want every diagnostic code named by a test, so that a change that stops
    emitting one is caught.

## Implementation decisions

- G-7 is removed from the specification by a change the maintainer lands; ADR-0008, ADR-0015, and
  ADR-0016, which name pushdown and G-7, stay as accepted and are named by the change as the ADRs
  whose basis it moves. The identifier is not reused; the gap stays until the next normalization
  renumbers.
- SVG symbols become a non-goal, by a change the maintainer lands in the line of the Non-Goals that
  names the pixmap formats.
- The cases of a non-goal leave the scope with a reason that names the non-goal; the header of the
  list says that a non-goal is a reason.
- A TrueType symbol is drawn from the glyph outlines the label path already reads (ADR-0031): the
  symbolizer measures and places the glyph as MapServer's marker code does, and the renderer fills
  it as it fills a label's glyph.
- A vector symbol as a polygon fill is rendered into a fill tile, held to the bound Q-9 gives a fill
  tile, as a pixmap's tile is.
- A circle layer, a style's `ANTIALIAS`, and the map's `ANGLE` are carried into the style IR, and a
  class's style draws its `GEOMTRANSFORM` expression. Each is a change of the style document, under
  ADR-0014 and with the maintainer (ADR-0006).
- The map's `ANGLE` is the last work of the plan, as the maintainer asked. It is cut into two
  tickets: the rotation of the map (13) and the symbols turned with it (14). Ticket 13 waits for
  every other ticket.
- `buffer`, `simplify`, and `simplifypt` become nodes of the transform tree once an ADR picks the
  geometry code that computes them; a style's drawn tree is evaluated on the shape in pixels, as
  msDrawTransformedShape does.
- The map's `ANGLE` reaches the shapes as MapServer's does, through the geotransform its
  reprojection applies: every layer reprojects when the map is rotated, and the area asked of a
  source is the rotated extent's bounds. Whether this fits ADR-0037's reprojection or changes its
  seam is decided before it is built.

## Testing decisions

- The msautotest harness, at the seam it has: each case flips from failing to passing in the ticket
  that draws it, and holds the comparison of G-9 on wasm32. The prior art is every case already
  passing.
- Below the harness, the existing test seams of each crate: the frontend's `load` with files in
  memory, the symbolizer's unit tests, the engine's render with sources in memory, the schema test
  of the style document.
- The browser test opens the example with the bundle served and checks the image and the diagnostics
  shown; its tool is chosen by an ADR.
- Each diagnostic code is named by a test of the engine's render or the style document, through the
  call that pushes it.

## Out of scope

- The other cases of misc and renderers that are out of scope for a category's exclusion: other
  output formats, OGC requests, raster sources, labels on lines and in polygons, complex script.
- Pushdown of a filter to a source.
- The categories raster and text (G-13) beyond what a geometry case needs.
- SVG symbols, GIF and other non-PNG pixmaps, chart layers, a layer's `GRID`, a symbol fetched by
  its URL, and the `ENCODING` of a label and of a layer, all non-goals of the specification.
- Projections `proj` does not implement yet, unless a case of this spec needs one (ADR-0037).

## ADR impact

- Binding: ADR-0002 (`Verifies:` markers), ADR-0005 and ADR-0006 (the plan, the modes, the style
  document never unattended), ADR-0007 (tools), ADR-0009 and ADR-0010 (the Dev Container and CI, for
  the browser test), ADR-0008 (crates and their edges), ADR-0011
  (dependencies), ADR-0013 and ADR-0014 (the style document's new fields), ADR-0017 (C's ports),
  ADR-0025, ADR-0026, ADR-0027, ADR-0042 (the renderer and its clip), ADR-0029 (fill tiles and
  images), ADR-0031 (glyphs), ADR-0033 and ADR-0034 (the harness and G-9), ADR-0035 (the example's
  vendored files), ADR-0048 (the transform tree), ADR-0037 (reprojection), and ADR-0019 and
  ADR-0038 (the resolver and the projection references) if ticket 14 ports `somerc`.
- New `proposed` ADRs before implementation:
  - the tool of the browser test of G-14 (ticket 02);
  - the geometry code behind `buffer`, `simplify`, and `simplifypt`, with ADR-0048, whose
    *Decision* lists the transform tree's nodes (ticket 11);
    ADR-0048 supersedes ADR-0043, and ADR-0043 superseded ADR-0036;
  - the map's rotation, if it changes the seam of ADR-0037's reprojection (ticket 13).
- Specification changes the maintainer lands: G-7 removed (ticket 01); SVG symbols as a non-goal,
  and a layer's `ENCODING` (ticket 04).
