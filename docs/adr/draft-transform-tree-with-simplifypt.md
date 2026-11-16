# ADR-draft-transform-tree-with-simplifypt: A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, `simplify`, `simplifypt`, and `buffer` among its nodes

- **Status:** 🟡 proposed
- **Date:** 2026-11-16
- **Deciders:** Markus Brigl
- **Applies to:** the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.1.json`
- **Supersedes:** [ADR-0043](0043-transform-tree-with-simplify-and-buffer.md)

## Context

[ADR-0043](0043-transform-tree-with-simplify-and-buffer.md) holds a `GEOMTRANSFORM` expression as a
typed tree of its shape functions, `simplify` and `buffer` among its nodes. It superseded
[ADR-0036](0036-typed-geomtransform.md), and its *Context* tells how. Its *Decision* keeps
`simplifypt` in the closed list of reasons a tree is not drawn. Its *Consequences* ask for a
successor once the backlog entry `docs/backlog/simplifypt-topology-preserving.md` is decided.

On 2026-11-16 the maintainer decided that entry: `simplifypt` is drawn, by a port of JTS's
`TopologyPreservingSimplifier`. The code is
[ADR-draft-simplifypt-ported-from-jts](draft-simplifypt-ported-from-jts.md). The tree must hold
`simplifypt` as a node with its argument. That changes ADR-0043's list of nodes and its closed
list of reasons, so this ADR supersedes it. The style `simplifypt([shape],50)` of
`renderers/geomtransform-vertex.map` is the last piece that case lacks.

This ADR restates every clause of ADR-0043 in its wording, but these:

- *Context*: ADR-0043's account of how it superseded ADR-0036 gives way to this account. Its
  paragraphs on the grammar, a class style's expression, msautotest, and versions are restated;
- *Decision*, its first sentence: it names `simplifypt` among the nodes;
- *Shape*: the node `simplifypt` is added;
- *Order of landing*: `simplifypt` joins `simplify` and `buffer`;
- *Not drawn*: `simplifypt` stays a reason only until its node lands;
- *Out of scope*: the code of `simplifypt` is added after the code of the two other functions.
  The examples of a not-drawn function to port are `centerline` and `difference`, in place of
  `centerline` and `simplifypt`;
- *Alternatives considered*:
  - a new entry, `simplifypt` kept a reason;
  - the entry on geo's `SimplifyVwPreserve` names the port that computes this ADR's node;
  - the entry on keeping the record and adding the node names ADR-0043 for ADR-0036;
  - the entry on keeping the reasons after the nodes land names `simplifypt` too;
  - ADR-0036's alternative of a not-drawn function's arguments: three functions drawn, for two;
  - the entry on a successor for the drawn style names ADR-0043 where it named "this ADR";
- *Consequences*: a new Positive item, the style of `simplifypt` drawn. The Positive item on the
  wording kept names ADR-0043 for ADR-0036. The item on the schema names `simplifypt` third. The
  Negative item on `simplifypt` left not drawn is removed;
- *Enforcement*: the tests of the tree and of the frontend name `simplifypt`. The tests that cite
  ADR-0043 move their `Verifies:` to this ADR, in place of those that cited ADR-0036.

**The grammar of the three** (`src/mapparser.y` at `rel-8-6-6`):

- `buffer(s, n)` (`:758`) calls `msGEOSBuffer(s, n)`, a distance in the shape's unit, which may
  be negative;
- `simplify(s, n)` (`:842`) calls `msGEOSSimplify(s, n)`, a tolerance in the shape's unit;
- `simplifypt(s, n)` (`:856`) calls `msGEOSTopologyPreservingSimplify(s, n)`, a tolerance too.

Each `n` is the rule `math_exp`, as for `generalize` and `densify`. MapServer fixes the 30
segments a quadrant of `buffer` in the C; no argument carries them.

**A class style's expression** (`rel-8-6-6`):

- `msDrawShape` (`src/mapdraw.c`) needs the unclipped shape where a style of a line or a
  polygon layer has a `GEOMTRANSFORM` and the shape is no point (`:2429-2437`). It transforms the
  shape to pixels (`:2505`) and copies it before it clips (`:2516-2518`). `lineLayerDrawShape`
  (`:2140-2148`) and `polygonLayerDrawShape` (`:2304-2311`) hand that unclipped pixel shape to
  `msDrawTransformedShape`, for each style in its scale range. `pointLayerDrawShape`
  (`:2034-2085`) draws a point layer's styles at its points whatever their transform.
- `msDrawTransformedShape` (`src/mapgeomtransform.c:198-240`) evaluates the style's expression
  on that shape with `yyparse`. It draws a line result with `msDrawLineSymbol`, and a point or a
  polygon result with `msDrawShadeSymbol`, each with the style itself and its scale factor. A
  result of another type draws nothing.
- A failure of the evaluation returns `MS_FAILURE`, as `msGeomTransformShape`'s does for a
  layer. `msDrawVectorLayer` stops at it (`src/mapdraw.c:1402-1405`, `:1427-1434`), and
  `msDrawMap` makes no map (`:508-519`). A failure of the drawing is not returned: the inner
  `int status` of the `case` shadows the function's.
- `msDrawTransformedShape` sets neither `p.dblval` nor `p.dblval2` of its local `parseObj`.
  These bind `[map_cellsize]` and `[data_cellsize]` (`src/mapparser.y:1121-1128`).
  `msGeomTransformShape` sets both for a layer (`src/mapgeomtransform.c:283-291`). So a style's
  `[map_cellsize]` reads an uninitialised double at the pin. No style of msautotest at 7515cccf
  uses it.

**What msautotest uses** at 7515cccf: the layer `vertex2` of `renderers/geomtransform-vertex.map`,
`simplify([shape],25)`; three styles of the same file, `simplify([shape],10)`,
`simplifypt([shape],50)`, and `generalize([shape],50)`; and two styles of
`renderers/polygon-as-line-offset.map`, `buffer([shape], 1)` and `buffer([shape], -1)`. No case
nests these functions or gives them arithmetic.

**Versions.** No release tag holds a schema yet. [ADR-0014](0014-style-document-versions.md),
*Before a release*, lets the change that alters the IR regenerate the current version's file,
`schema/style-document-1.1.json`. For the fidelity plan the maintainer chose to keep the version
at 1.1 and regenerate its file, as ticket 09 added `RenderMode::Circle` and `Style.aliased` to
1.1. A version move stays the maintainer's choice, change by change.

## Decision

We will hold a `GEOMTRANSFORM` expression in the style IR as a typed tree of its shape functions,
`simplify`, `simplifypt`, and `buffer` among its nodes. Only the mapfile frontend translates it
from MapServer's text.

- **One type, two places.** The tree is one type of `crates/style-ir`. A layer holds it in the
  optional field `geom_transform`. A class's style holds it in `GeomTransform::Expression`, which
  holds no `String`.
- **Shape.** The tree is written in [ADR-0015](0015-expressions-as-cql2.md)'s CQL2-JSON shape,
  `{"op": name, "args": [...]}`. Its nodes are:
  - `shape`, without arguments, the feature's shape, MapServer's `[shape]`;
  - `generalize` and `densify`, each of a shape and a number;
  - `simplify`, of a shape and a number, the tolerance;
  - `simplifypt`, of a shape and a number, the tolerance;
  - `buffer`, of a shape and a number, the distance;
  - `outer` and `inner`, each of a shape;
  - `smoothsia`, of a shape and up to three further arguments: a size, a number of iterations,
    and a preprocessing of `all` or `angle`.
- **Order of landing.** Each of `simplify`, `buffer`, and `simplifypt` must enter the tree in the
  change that draws it. Until that change, the frontend must translate the function to its
  `not_drawn` reason. That change must remove the function from the list of reasons.
- **Numbers.** A number argument is a JSON number, `map_cellsize` without arguments, or `+`, `-`,
  `*`, `/`, `%`, or `^` of two number arguments.
  - The frontend must fold every operation without `map_cellsize` to one JSON number at load.
  - The frontend must fold `[data_cellsize]` to the number 0, as `mapgeomtransform.c` binds it on
    every layer but a contour layer, which Forager does not carry.
  - The IR has no negation. The frontend must translate MapServer's unary `-` to its operand, as
    `mapparser.y` does.
  - The frontend must translate a text `smoothsia` preprocessing other than `all` or `angle`, read
    without regard to case, to no preprocessing.
  - The tree must hold a negative tolerance or distance as it is written. Its evaluation decides
    what it draws.
- **Not drawn.** A transform the engine does not draw is the whole tree `{"not_drawn": reason}`.
  The reason is one name of a closed list:
  - each function of `shape_exp` and `math_exp` the tree lacks: `centerline`, `difference`,
    `javascript`, `length`, `area`, `round`, and each of `buffer`, `simplify`, and `simplifypt`
    until it enters the tree;
  - `binding`, for a property's value where a number or a text is due;
  - `text`, for a computed `smoothsia` preprocessing;
  - `fromtext`, for a shape `fromtext` reads from a text;
  - `literal`, for an operation of literals that folds to no finite JSON number, a `%` by 0 among
    them. A single literal past a double, such as `1e999`, which `atof` reads as infinite, is one
    too;
  - `syntax`, for an expression `shape_exp` does not parse, a `/` by 0 among them.

  A syntax error takes precedence: MapServer refuses the whole expression, so an expression with a
  syntax error is `syntax`. Otherwise the frontend must name the first such construct in the
  expression's reading order. The tree must hold no text of the mapfile.
- **A class's style.** The engine must draw a class style's tree on a shape of a line or a
  polygon layer as `msDrawTransformedShape` draws it.
  - The engine must evaluate the tree on the shape in pixels, unclipped, as `msDrawShape` hands
    it on.
  - It must bind `[map_cellsize]` to the value a layer's `GEOMTRANSFORM` binds it to. MapServer
    binds none for a style at the pin.
  - It must draw a line result as `msDrawLineSymbol` draws it. It must draw a point or a polygon
    result as `msDrawShadeSymbol` draws it. Each takes the style itself.
  - It must draw the style in the pass in which it draws a style of a keyword transform, such as
    `vertices`, as `lineLayerDrawShape` hands both to `msDrawTransformedShape`.
  - It must draw a point layer's style at the layer's points, whatever its tree, as
    `pointLayerDrawShape` does.
  - The evaluations of a layer's style trees must charge the bounds of Q-9 that the layer's
    `GEOMTRANSFORM` charges: the points added and the points visited. One budget holds for the
    layer and its styles in a render.
  - Once an evaluation passes a bound, the layer's style trees must draw nothing on that shape and
    on every later shape of the layer. The shapes drawn before stay drawn.
  - Where the evaluation fails on a shape, the style must draw nothing on that shape. MapServer
    makes no map there. The layer's other styles and the rest of the map are drawn.
  - For a failure, the engine must warn `geomtransform-not-drawn` once for each failing style and
    render. The warning must name the style's first failure, in the order the shapes are drawn.
  - Past a bound, the engine must warn `geomtransform-over-limit` once for the layer and render,
    whatever the number of style trees the bound stops.
- **Who parses.** Only `crates/mapfile` may parse a `GEOMTRANSFORM` expression. The core must read
  the tree and never MapServer's text.
- **Diagnostics.** The engine must warn `geomtransform-not-drawn` for a `not_drawn` tree, naming
  its reason, of a layer and of a class's style alike. A class's style of a `not_drawn` tree must
  draw nothing. The frontend must warn at load only of a syntax error, with its place.
- **Bounds.** The tree must nest at most `MAX_EXPRESSION_DEPTH`, 24 levels, counted as ADR-0015
  counts an expression's depth.
  - The frontend's parser must count each operator of a chain as one level of the tree it builds.
  - It must refuse a deeper tree before it recurses deeper, without overflowing the stack.
  - A deeper tree is the load error `expression-too-deep`, from a mapfile and from a style
    document alike (Q-9).
- **Versions.** The tree entered the document at 1.0, the version of that day, and
  `schema/style-document-1.0.json` was regenerated in the change that implemented ADR-0036. The
  current version's file, `schema/style-document-1.1.json`, holds it today. A function added to
  the tree after the first release must be a major version. A reader of an older minor unsets an
  unknown function, and so draws the shapes untransformed. It must read a `not_drawn` reason it
  does not know as not drawn, with a warning naming the reason and its path. The tree holds such a
  reason as `syntax`, so the engine's later `geomtransform-not-drawn` names `syntax`. A reason
  added in a minor version so fails closed.
  - Before a release, the change that adds a node must regenerate the current version's file.
  - The format version stays 1.1 unless the maintainer moves it. A move must follow ADR-0014's
    version rule.
  - The change that draws a class style's tree must change the doc comment of
    `GeomTransform::Expression` from not drawn to drawn. That comment is the schema's
    description, so the change must regenerate `schema/style-document-1.1.json`. The version
    stays 1.1.

**Out of scope:**

- the evaluation of the tree and each function's semantics, which `docs/semantics/` writes down
  from the C, `[map_cellsize]` and the layer's `UNITS` among them;
- the code that computes `simplify` and `buffer`, which
  [ADR-0041](0041-geometry-through-geo.md) decides;
- the code that computes `simplifypt`, which
  [ADR-draft-simplifypt-ported-from-jts](draft-simplifypt-ported-from-jts.md) decides;
- the bounds on the points a transform adds and visits beyond those Q-9 names;
- a label's style of an expression, which keeps `LabelTransform` and is left out with its label as
  today;
- porting any function of the not-drawn list, `centerline` and `difference` among them.

## Alternatives considered

- **Keep ADR-0043 and add the node under it** — no new record. ADR-0043's *Decision* lists the
  nodes and the closed list of reasons, so adding a node changes an accepted decision. An accepted
  ADR changes only by supersession ([`AGENTS.md` §3](../../AGENTS.md#3-adr-rules), rule 5).
- **One node for both simplifications, with a flag for topology** — one name fewer. MapServer's
  grammar has two functions, and the tree mirrors `shape_exp` function by function. A flag would
  be a field no CQL2-JSON function has.
- **`buffer` with its segments a quadrant as a third argument** — open for a style document that
  wants fewer. No mapfile can write it, and MapServer fixes 30. It can come later as an optional
  argument, which ADR-0014's version rule allows as a minor.
- **Both nodes at once, before the engine draws `buffer`** — one schema change instead of
  two. The tree would hold a node the engine reads as not drawn. ADR-0036 rejected nodes the engine
  never reads, and a `not_drawn` reason says the same with less.
- **`simplifypt` as a node computed by geo's `SimplifyVwPreserve`** — the case's style would draw.
  Visvalingam-Whyatt is another algorithm with an area as its tolerance, and it differs from GEOS
  on 4 of the case's 6 line results. A node would carry a meaning the reference does not have.
  The node is computed by the port of
  [ADR-draft-simplifypt-ported-from-jts](draft-simplifypt-ported-from-jts.md) instead.
- **Keep `simplifypt` a reason** — ADR-0043's line, no change of the schema. The style
  `simplifypt([shape],50)` of `renderers/geomtransform-vertex.map` stays undrawn, and its case
  cannot pass.
- **Keep `buffer`, `simplify`, and `simplifypt` in the list of reasons after their nodes land** — a
  document written by an older build of 1.1 with such a reason stays readable. No release holds
  1.1, so no published document has one. The reason would name a function the engine draws.
- **Move the format version to 1.2** — ADR-0014's version rule allows an added function as a
  minor, while this tree's *Versions* asks a major for a function added after the first release.
  Before a release ADR-0014 leaves the move to the maintainer, who chose to keep 1.1 for this
  plan.
- **The alternatives of ADR-0036** — MapServer's text in the IR, a unit variant `Unsupported`, the
  style left out at load, a chain instead of a tree, the numbers as ADR-0015 expressions, literal
  numbers only, and superseding ADR-0015. They stand as ADR-0036 records them. Its alternative of
  a not-drawn function kept in place with its arguments stands for the functions still not drawn;
  for the three drawn here the engine now reads the arguments.
- **Keep a class style's expression not drawn** — ADR-0036's line, no change of meaning. The
  styles of `generalize` and `simplify` in `renderers/geomtransform-vertex.map` stay undrawn, and
  its case cannot pass.
- **A successor of its own for the drawn style** — a record for each decision. The drawn style
  changes the meaning of the type ADR-0043 decides, `GeomTransform::Expression`, and ADR-0036
  excluded it in the clause ADR-0043 restates. A second successor would supersede ADR-0036 twice.
  The maintainer chose ADR-0043 on 2026-11-15.
- **The style's tree evaluated in map units, before the pixels, as a layer's** — one path of
  evaluation. MapServer evaluates a style's tree on the shape in pixels, so its numbers are pixels:
  the case's `generalize([shape],50)` keeps, of the shape's first part alone, its first point,
  each point at least 50 pixels from the last kept, and its last point (`msGeneralize`,
  `src/maputil.c:2823-2871`).
- **A style's `[map_cellsize]` as 1, a cell in pixels** — a value that fits the pixel shape. The
  pin binds none, so no reference decides. The layer's value keeps one meaning for one name and
  needs no rule of its own.
- **A `not_drawn` reason for a style's `[map_cellsize]`** — true to the read of an uninitialised
  double. It adds a reason to the closed list, and the frontend's translation would depend on
  whether the tree sits on a layer or a style.
- **A budget of its own for each style, or for each shape** — a style's evaluation cannot starve
  the layer's. A layer of many styles, or many shapes, would multiply the bound of Q-9. The
  maintainer chose one budget a layer and render.
- **The whole layer undrawn where a style's evaluation fails, as for a layer's transform** — nearer
  MapServer, which stops the layer and makes no map. The engine could evaluate the style trees
  over all features before it draws, as it does the layer's transform. It would hold every
  style's result for the layer at once, and drop the layer's other styles for one style's failure.
  The canvas's budget of markers already stops a layer's markers part way, so a partial draw has a
  precedent. Neither form is MapServer's.

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapparser.y` (`:758`, `:842`, `:856`) and `src/mapgeos.c`
  (`:899`, `:922`, `:946`), read 2026-11-15 from `target/ms-src`.
- The msautotest suite at 7515cccf: `renderers/geomtransform-vertex.map` and
  `renderers/polygon-as-line-offset.map`, read 2026-11-15.
- [ADR-0036](0036-typed-geomtransform.md), whose *Context* and *Sources* this ADR keeps.
- [ADR-0014](0014-style-document-versions.md), [ADR-0015](0015-expressions-as-cql2.md),
  [ADR-0008](0008-ports-and-adapters-crates.md), [ADR-0017](0017-c-ports-in-style-ir.md).
- MapServer `rel-8-6-6`, `src/mapgeomtransform.c` (`msDrawTransformedShape`, `:115-250`;
  `msGeomTransformShape`, `:283-291`), `src/mapdraw.c` (`:508-519`, `:1402-1405`, `:2034-2085`,
  `:2140-2148`, `:2304-2311`, `:2429-2437`, `:2505-2518`), `src/mapparser.y` (`:1121-1128`), and
  `src/maputil.c` (`msGeneralize`, `:2823-2871`), read 2026-11-15 from `target/ms-src`.
- `docs/plan/fidelity/10-style-geomtransform-expressions.md`,
  `docs/plan/fidelity/11-simplify-through-a-geometry-library.md`, and
  `docs/plan/fidelity/12-geomtransform-buffer.md`.

## Consequences

- Positive: a layer of `simplify` draws, and the case of `vertex2` can pass. A style document may
  write both functions as CQL2-JSON custom functions.
- Positive: a class style's tree draws as MapServer draws it, on a line or a polygon layer. The
  styles of `generalize` and `simplify` in `renderers/geomtransform-vertex.map` draw.
- Positive: a tree of `simplifypt` draws. The style `simplifypt([shape],50)` of
  `renderers/geomtransform-vertex.map` draws, and its case can pass.
- Positive: every clause of ADR-0043 but those the *Context* lists keeps its wording, so the
  frontend's tests and the engine's rules of a layer's `not_drawn` tree stand unchanged.
- Negative / trade-offs: a document of 1.1 written before a node lands, with that function's
  reason, fails to load after it. No release holds 1.1, so only local files are affected.
- Negative / trade-offs: the schema changes with each change that lands a node, `simplify` first,
  `buffer` second, and `simplifypt` third, and with the change that draws a class style's tree.
- Negative / trade-offs: a document of 1.1 written before that change, with a class style's tree,
  draws the style after it, where it drew nothing. No release holds 1.1, so only local files are
  affected.
- Negative / trade-offs: a style's `[map_cellsize]` and a style's failed evaluation deviate from
  the pin, which reads an uninitialised double and makes no map.
- Negative / trade-offs: a layer's style trees share one budget with its `GEOMTRANSFORM`. A layer
  of many such styles reaches the bound sooner. Past it, its style trees draw on the earlier shapes
  and not on the later ones, so what is drawn depends on the order of the features.
- Follow-ups:
  - the semantics page of the layer's transform, with `[map_cellsize]`, the layer's `UNITS`, and
    GEOS's failures;
  - the semantics page of a class style's tree, in pixels, with its `[map_cellsize]` and its
    failures;
  - Q-9's bound shared by a layer's style trees, a change to the specification the maintainer
    lands.

## Enforcement

- Tests of the transform tree in `crates/style-ir` cite this ADR with `Verifies:`. They read and
  write the CQL2-JSON shape of each node, number, and `not_drawn` reason, `simplify`, `buffer`,
  and `simplifypt` among the nodes once they land.
- A test of `crates/style-ir/src/document.rs` cites this ADR and Q-9 with `Verifies:`. It refuses
  a document whose transform tree nests past `MAX_EXPRESSION_DEPTH`.
- A test of `crates/style-ir/tests/schema.rs` cites this ADR with `Verifies:`. It walks the schema
  of the transform tree and of `GeomTransform`. It fails on any string that is not a value of a
  closed enum. This decides by type that the tree holds no mapfile text.
- Tests of the mapfile frontend cite this ADR with `Verifies:`. They translate each expression of
  ADR-0036's table to its tree or its reason. They cover:
  - the folding of literal arithmetic, and `map_cellsize` kept;
  - unary `-` left un-negated;
  - `simplify([shape],25)` translated to its node once it lands;
  - `simplifypt([shape],50)` translated to its node once it lands;
  - a chain of 25 `+` refused as `expression-too-deep`, without a stack overflow (Q-9);
  - a syntax error warned with its place.
- A test of `crates/engine` cites this ADR with `Verifies:`. A layer of a `not_drawn` tree draws
  nothing and warns `geomtransform-not-drawn` with its reason.
- Tests of `crates/engine` cite this ADR with `Verifies:`. They render with sources in memory:
  - a line layer's style of `generalize` strokes the generalized line in pixels, without a
    warning;
  - a style whose tree yields a polygon is shaded as `msDrawShadeSymbol` shades it;
  - a class style of a `not_drawn` tree draws nothing and warns `geomtransform-not-drawn` with its
    reason;
  - a style whose evaluation fails on a shape draws nothing on it, with its warning;
  - a style whose tree fails on two shapes for different reasons warns once, naming the first;
  - a style of `[map_cellsize]` evaluates it at the layer's value;
  - a point layer's style of a tree draws its symbol at the layer's points.
- Tests of `crates/engine` cite this ADR and Q-9 with `Verifies:`. A style's tree visits past the
  budget its layer's `GEOMTRANSFORM` left, or adds points past it. The style draws nothing on that
  shape and the later ones. A layer of two such style trees warns `geomtransform-over-limit` once.
- The tests that cite [ADR-0043](0043-transform-tree-with-simplify-and-buffer.md) move their
  `Verifies:` to this ADR in the change that implements it.

**Not mechanically decidable:** that no core crate parses MapServer's text by another route, such
as a string field added elsewhere. The schema test covers the transform tree only. Review catches
the rest.
