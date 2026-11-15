# Ticket: Circle layers

## What to build

A layer of `TYPE CIRCLE` draws each feature as the circle MapServer draws from it. Today the
frontend reads `CIRCLE` as the render mode polygon, so the engine draws the feature's points as a
polygon, or nothing. MapServer sends a circle layer's shapes through `circleLayerDrawShape`
(`src/mapdraw.c:1988`, called at `:2389`). A shape of exactly one part of exactly two points is a
circle: its centre is the midpoint of the two points, and its radius is half their distance in x, or
in y where that is 0. Any other shape, and a radius of 0, draws nothing and fails nothing. The
centre is reprojected and turned into pixels, the radius divided by the cellsize. Each style in its
scale range is drawn by `msCircleDrawShadeSymbol` (`src/maprendering.c:1183`).

The render mode becomes `circle` in the style IR, an enum value the version rule of ADR-0014 allows
as a minor change. No release tag holds a schema yet, so the change regenerates
`schema/style-document-1.1.json`, unless the maintainer moves the version. A reader of an older
minor unsets the unknown value. The frontend keeps giving a circle layer's features the shape type
point, as `loadLayer` does.

Case: `renderers/circle_layer.map circle_layer.png`: two features, one inside the extent and one
whose circle reaches beyond it.

## Acceptance criteria

- [ ] Given `renderers/circle_layer.map`, when the harness renders `circle_layer.png`, then it
  passes by the metric of G-6, natively and bit-identically on wasm32 (G-9), and the list says
  `geometry passes`.
- [ ] Given a mapfile layer of `TYPE CIRCLE`, when the frontend loads it, then its render mode is
  `circle`, and the style document round-trips through YAML and JSON (G-3).
- [ ] Given the style IR, when the schema test runs, then the checked-in schema matches the derived
  one.
- [ ] Given a circle layer's feature of other than one part of two points, or of radius 0, when it
  is drawn, then nothing is drawn for it and the layer's other features draw, as
  `circleLayerDrawShape` skips it.
- [ ] Given every case in scope, when they are rendered, then each holds its outcome.

## Read first

- Touches: `crates/style-ir/src/model.rs` (`RenderMode`), `schema/style-document-1.1.json`,
  `crates/mapfile/src/translate/mod.rs` (`layer_type`), `crates/engine/src/lib.rs` (`drawn`,
  `drawable_mode`), `crates/engine/src/layer.rs`, `crates/symbolize/src/draw.rs`,
  `crates/cli/tests/conformance/cases.txt`
- MapServer at the pin: `src/mapdraw.c` (`circleLayerDrawShape`, `msDrawShape`),
  `src/maprendering.c` (`msCircleDrawShadeSymbol`), `src/mapfile.c` (`loadLayer`'s `TYPE`)
- Glossary: Render mode, Style document, Format version
- `docs/adr/0014-style-document-versions.md` (*Decision*: the version rule, *Before a release*),
  `docs/adr/0013-style-document-serde.md`
- Golden path: the polygon layer's draw in `crates/symbolize/src/draw.rs`
- Seam: the msautotest harness; the frontend's `load` with files in memory;
  `crates/style-ir/tests/schema.rs`; the engine's render with sources in memory

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document, which ADR-0006 never leaves unattended. The session
stops before it commits the change to `RenderMode` and the schema, and asks the maintainer whether
it is a minor change of 1.1 or a version move. It also stops when the case still fails after two
attempts at its remaining difference.
