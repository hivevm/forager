# A layer's GEOMTRANSFORM is not carried

Observed: the frontend warns of a layer's `GEOMTRANSFORM` with `unsupported-keyword` and draws the
layer's shapes as read, where MapServer replaces each shape it reads by the expression
(`msLayerNextShape`, `msGeomTransformShape`, RFC 89). The maintainer decided on 2026-11-10 to carry
it, porting `generalize`, `densify`, `outer`, `inner`, and `smoothsia`, those that need no geometry
library, and drawing nothing of a layer of any other function, with a warning.

The ticket `maxfeatures-and-layer-geomtransform/01` built it, and the review of the branch
`feat/stable-2026-11-12` found it at odds with the specification and ADR-0015: the style IR held
MapServer's expression as text, and the core parsed it, so only the frontend did not know its
syntax, and a syntax error passed the load unreported. Its parser also bounded no chain of `+ - *
/ %`, and a long one overflowed the stack. The maintainer chose on 2026-11-12 to take it out of that
branch. The port is in the commit "feat(mapfile,engine,symbolize): MAXFEATURES and the
GEOMTRANSFORM expressions of styles and layers (maxfeatures/01)", 44cc731 since that branch was
folded into one commit per ticket: `crates/symbolize/src/layer_transform.rs`
and its `parse.rs` and `tests.rs`, the tests' values derived from the C.

A later change has the frontend translate the expression into a tree of the IR, the functions and
their numeric arguments, `[map_cellsize]` among them, every chain counted against the nesting bound
(Q-9). It also decides what the review left open: the layer's `UNITS`, which `[map_cellsize]`
reads; that MapServer transforms a shape only as it reads it, so a feature past `MAXFEATURES` fails
nothing; a multipoint, which the IR flattens; a quoted attribute binding, which MapServer reads; and
the two bounds of Q-9 the port added, on the points added and visited.

Where: crates/mapfile/src/translate/mod.rs (the layer's keywords), crates/style-ir/src/model.rs
(`Layer`), crates/engine/src/lib.rs (the features of a layer), crates/symbolize,
docs/semantics/style-symbol.md, docs/SPECIFICATION.md (Q-9).
