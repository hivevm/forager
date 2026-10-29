# Spec: Geometry styles and symbols

## Problem

Of the 58 msautotest cases of the category geometry and symbols that the harness renders, 4 pass.
The frontend reads nearly every property of a style into the style IR, but the engine draws only
filled polygons, stroked lines, and plain point marks, with the first class of each layer for
every feature. A map whose classes are chosen by `EXPRESSION`, whose points are vector or ellipse
symbols, whose lines are dashed, offset, or marked with symbols, or whose features are written
inline in the mapfile is drawn wrong or not at all.

## Solution

The maintainer and a user see the geometry of a map drawn as MapServer draws it: each feature in
the class its expression selects, past the layer's filter, with the symbols, dash patterns, caps,
joins, offsets, outlines, opacity, and geometry transforms of its styles, including the features a
mapfile writes inline. The geometry cases of the harness flip from failing to passing as each piece
lands.

## Existing behaviour

- The frontend translates `EXPRESSION`, `FILTER`, and every style property listed above into the
  style IR, with MapServer's typing made explicit (ADR-0030); its tests pin that.
- The engine fits the extent, queries each layer's source, and draws every feature with each
  style of the first class by the layer's render mode; its render tests pin points, lines,
  polygons, holes, and the line width cap with Forager's own reference images. Those stay.
- The harness lists each failing geometry case with what it lacks; a case flips to passing in the
  change that makes it pass.
- A layer's inline `FEATURE` blocks and a layer's `OPACITY` are not carried into the IR.

## Goals served

- G-6, for the category geometry and symbols: the share of passing cases grows.
- Q-4: the fidelity record only moves forward.
- G-9 and Q-2: every new drawing path renders the same bytes natively and in the browser.

## User stories

1. As a user, I want each feature drawn with the class whose `EXPRESSION` matches it, and only the
   features that pass the layer's `FILTER`, so that my map shows the classes MapServer shows.
2. As a user, I want a layer's inline `FEATURE` points, lines, polygons, and WKT drawn, so that a
   mapfile without a data file renders.
3. As a user, I want vector and ellipse symbols at points, rotated by `ANGLE`, and sized by `SIZE`,
   bound or computed, so that my markers look as in MapServer.
4. As a user, I want symbols placed along lines by `GAP` and `INITIALGAP`, so that marker lines
   render.
5. As a user, I want `PATTERN`, `LINECAP`, `LINEJOIN`, `OFFSET`, and `OUTLINEWIDTH` of line
   styles honoured, so that dashed, offset, and outlined lines match.
6. As a user, I want `OPACITY` of a style and of a layer, and a transparent background with
   `IMAGEMODE RGBA`, so that translucent maps match.
7. As a user, I want hatch symbols filling polygons, so that hatched areas render.
8. As a user, I want `GEOMTRANSFORM` (`start`, `end`, `vertices`, `centroid`, and the layer's),
   `POLAROFFSET`, and `MAXFEATURES` honoured, so that those cases match.
9. As the maintainer, I want each case these make pass flipped in the list in the change that makes
   it pass, so that the record shows the progress.

## Implementation decisions

- **Evaluation.** The core crate `expr` of ADR-0009 evaluates the IR's CQL2 tree against a
  feature's properties as MapServer evaluates its expressions: `atof` for `to_number`, string
  comparison as `strcmp`, `casei`, `in`, the arithmetic, and the string functions. Its regular
  expressions, for `regexp_like`, are read in PCRE2's syntax in byte mode (ADR-0030) and matched
  as ADR-0031 decides. The engine selects the first class whose
  expression matches, after the layer's filter, and evaluates bound and computed style
  properties per feature.
- **Inline features.** A layer's `FEATURE` blocks (`POINTS`, `WKT`, `ITEMS`) reach the engine as a
  source the frontend builds from the mapfile; where they live in the style IR, and how the style
  document carries them, is an ADR, since it changes the format.
- **Symbols.** The engine draws a style's symbol by its kind as MapServer's renderer does at the
  pin: vector symbols with their points, filled or stroked, ellipses, and hatch fills, each sized,
  rotated, and anchored as `src/maprendering.c` and `src/mapagg.cpp` do; symbols along a line are
  placed by `GAP` and `INITIALGAP` as `msDrawMarkerSymbol` and its line variant place them.
- **Line styles.** Dash patterns, caps, and joins go to agg-rust's stroker as MapServer's AGG
  renderer sets them; `OFFSET` on a line is MapServer's offset curve.
- **Opacity.** A style's `OPACITY` scales its colours' alpha as MapServer does; a layer's needs the
  layer drawn into its own image and composited, and its field in the IR.
- **Out of the spec's reach for now:** pixmap, SVG, and TrueType symbols, chart and circle layers,
  `GRID`, the map's `ANGLE`, and a `GEOMTRANSFORM` that needs a geometry library (`buffer`).

## Testing decisions

- Each drawing path is tested at the engine with Forager's own reference images, as the existing
  render tests are, and the evaluator at its crate on literal trees and properties.
- Each harness case a ticket makes pass flips to `passes` in the list in that ticket's change; a
  case still failing keeps its reason, updated to what it still lacks.
- Every new path runs in the native-and-wasm comparison once ticket 10 of the conformance harness
  lands.

## Out of scope

- Pixmap, SVG, and TrueType symbols; chart and circle layers; `GRID`; the map's `ANGLE`.
- A `GEOMTRANSFORM` that needs a geometry library, such as `buffer`.
- Labels, which have their own spec.

## ADR impact

- Binding: ADR-0009 (the `expr` crate and its edges), ADR-0012 (every new crate), ADR-0015 (the
  reference renderer), ADR-0030 (the tree the evaluator reads), ADR-0020 (the format change for
  inline features and layer opacity), ADR-0026 (the properties the evaluator reads), ADR-0007 (the
  format and the specification stay with the maintainer).
- New `proposed` ADRs before the tickets that need them: the evaluator and its regular
  expressions; inline features in the style IR.
