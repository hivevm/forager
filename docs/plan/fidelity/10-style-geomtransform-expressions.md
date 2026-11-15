# Ticket: A style's `GEOMTRANSFORM` expressions in `geomtransform-vertex.png`

## What to build

A class's style whose `GEOMTRANSFORM` is an expression draws the shape the expression computes, as
MapServer draws it. Today the engine warns that such a style is not drawn and draws nothing
(`styles_not_drawn` in `crates/engine/src/lib.rs`, the early return in
`crates/symbolize/src/draw.rs`). ADR-0036's *Out of scope* says the engine keeps drawing none.
Whether drawing them now needs a successor of ADR-0036 is the maintainer's to judge, not this
ticket's.

`msDrawTransformedShape` (`src/mapgeomtransform.c:115`, the expression at `:198-240`) evaluates the
expression on the shape already in pixels. It draws a resulting line with `msDrawLineSymbol` and a
point or polygon with `msDrawShadeSymbol`, with the style itself. The transform tree's evaluation
already exists for a layer's `GEOMTRANSFORM` (`crates/symbolize/src/layer_transform.rs`). The style
reuses it on the pixel shape, with `[map_cellsize]` bound as it is for a style.

The case `renderers/geomtransform-vertex.map geomtransform-vertex.png` (`-l "centroid vertex1"`)
needs more than this:

- the `vertices`, `start`, and `end` transforms with `ANGLE AUTO`, which exist, drawn with the
  TrueType symbols `segment`, `arrowhead`, and `arrowtail`. That needs ticket 05;
- styles of `generalize([shape],50)`, which the tree evaluates today, and of `simplify([shape],10)`
  and `simplifypt([shape],50)`, which are `not_drawn` until ticket 11 adds them;
- the `centroid` layer's ellipse marker, which exists.

So the case flips only after tickets 05 and 11. Its note in the list names a style's expressions and
TrueType symbols, but not `simplify` and `simplifypt`, which it also needs.

## Acceptance criteria

- [ ] Given `renderers/geomtransform-vertex.map`, when the harness renders
      `geomtransform-vertex.png`,
  then it passes by the metric of G-6, natively and bit-identically on wasm32 (G-9), and the list
  says `geometry passes`.
- [ ] Given a line layer's style of `GEOMTRANSFORM (generalize([shape], n))`, when the engine
      renders
  it, then the style strokes the generalized line in pixels, and no `geomtransform-not-drawn`
  warning or "not drawn" warning is given for it.
- [ ] Given a style whose expression yields a polygon or a point, when it is drawn, then it is
  filled or marked as `msDrawShadeSymbol` would draw it.
- [ ] Given a style whose tree is `not_drawn`, when it is drawn, then the engine still warns
  `geomtransform-not-drawn` with its reason and the style draws nothing.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/symbolize/src/draw.rs`, `crates/symbolize/src/layer_transform.rs`,
  `crates/symbolize/src/geom_transform.rs`, `crates/engine/src/lib.rs` (`styles_not_drawn`),
  `crates/style-ir/src/model.rs` (the doc comment of `GeomTransform::Expression`),
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapgeomtransform.c` (`msDrawTransformedShape`), `src/mapdraw.c`
  (`msDrawShape`, where it hands the pixel shape on), `src/mapparser.y` (the shape functions)
- Glossary: Transform tree
- `docs/adr/0036-typed-geomtransform.md` (*Decision*, *Out of scope*)
- Golden path: the layer's transform in `crates/symbolize/src/layer_transform.rs` and its tests
- Seam: the msautotest harness; the engine's render with sources in memory

## Blocked by

- docs/plan/fidelity/05-truetype-symbol-at-a-point.md
- docs/plan/fidelity/11-simplify-through-a-geometry-library.md

## Mode

with the human:

- It changes the meaning of the style document's `GeomTransform::Expression` on a class's style,
  from reported and not drawn to drawn. The doc comment of that variant is the schema's
  description (`schema/style-document-1.1.json`). ADR-0006 keeps the style document's format and
  its versions with the maintainer, and ADR-0014 decides whether a change of meaning is a major
  version.
- A style's tree that visits points is evaluated per shape, while Q-9 bounds only a layer's
  `GEOMTRANSFORM`. ADR-0006 keeps resource limits with the maintainer.
- The session stops before the commit that changes `crates/style-ir` or the schema. It hands the
  maintainer the change of meaning, the version question, the bound of Q-9, and whether ADR-0036
  needs a successor.
- It also stops when the case still fails after two attempts at its remaining difference. Then it
  commits the drawn styles with their tests, keeps the case `fails` with a corrected note, and
  hands off.
