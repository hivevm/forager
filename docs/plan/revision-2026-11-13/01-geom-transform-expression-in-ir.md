# Ticket: Decide whether the style IR may hold a transform as MapServer text

## What to build

`GeomTransform::Expression(String)` at `crates/style-ir/src/model.rs:648-654` keeps a class's
`GEOMTRANSFORM` expression, `(buffer([shape], 5))` and the like, as MapServer wrote it, only so that
the engine can report it as not drawn (`crates/engine/src/lib.rs:824`,
`crates/symbolize/src/draw.rs:235`, `crates/symbolize/src/geom_transform.rs:92`). The style
document thus carries MapServer syntax. ADR-0015 makes every expression of the IR a CQL2 tree, and
the overview names `mapfile` the only crate that knows MapServer syntax. No ADR admits an opaque
foreign text in the IR. The layer's `GEOMTRANSFORM`, held the same way, was taken out again in
7f36bf6 for this reason; the class's stayed.

Run `propose-adr` with the maintainer. On 2026-11-13 the maintainer chose the first option's
direction: a typed transform tree in the IR, which the frontend translates MapServer's expressions
into, and which brings back the layer's `GEOMTRANSFORM` of `docs/backlog/layer-geomtransform.md`.
The options the survey found:

- a typed tree of the transform's functions in the IR, by an ADR (chosen);
- an ADR that admits "an untranslated transform, kept by its source text" as a concept of the IR;
- a unit variant, such as `Unsupported`, with the text kept in the load's diagnostic alone;
- the style left out at load with its warning, as a label's style of an unknown transform is.

The ticket takes up `docs/backlog/layer-geomtransform.md`: a layer's `GEOMTRANSFORM`, which the
maintainer decided on 2026-11-10 to carry, porting `generalize`, `densify`, `outer`, `inner`, and
`smoothsia` and drawing nothing of a layer of any other function, with a warning. The port is in
44cc731 (`crates/symbolize/src/layer_transform.rs`, its `parse.rs` and `tests.rs`, the tests'
values derived from the C), taken out in 7f36bf6. The tree decides what that review left open: the
layer's `UNITS`, which `[map_cellsize]` reads; that MapServer transforms a shape only as it reads
it, so a feature past `MAXFEATURES` fails nothing; a multipoint, which the IR flattens; a quoted
attribute binding; every chain of `+ - * / %` counted against the nesting bound; and the two bounds
the port added on the points added and visited, which Q-9 does not name yet (a drafted
specification change, for the maintainer to land).

## Acceptance criteria

- [ ] Given the options above, when the maintainer has decided, then a `proposed` ADR records the
  decision in a commit of ADRs only, or the ticket records why none is due.
- [ ] Given the decision, when it is implemented on a branch after the ADR's commit, then the style
  document holds no MapServer expression text, or holds it as the ADR decides, and the schema
  `schema/style-document-1.0.json` is regenerated with it.

## Read first

- Touches: `crates/style-ir/src/model.rs`, `crates/mapfile/src/translate/style.rs`,
  `crates/engine/src/lib.rs`, `crates/symbolize/src/draw.rs`,
  `crates/symbolize/src/geom_transform.rs`, `schema/style-document-1.0.json`
- `docs/adr/0015-expressions-as-cql2.md`, `docs/adr/0014-style-document-versions.md`
- `git show 44cc731 -- crates/symbolize/src/layer_transform.rs crates/symbolize/src/layer_transform`

## Blocked by

- None (can start immediately)

## Mode

with the human: an ADR, and the style document format, which ADR-0006 keeps with the maintainer.
