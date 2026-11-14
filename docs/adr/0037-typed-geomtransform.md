# ADR-0037: A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, translated by the mapfile frontend

- **Status:** 🟢 accepted
- **Date:** 2026-11-13
- **Deciders:** Markus Brigl
- **Applies to:** the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.0.json`

## Context

A `GEOMTRANSFORM` may be an expression in parentheses, on a class's style and on a layer.
MapServer parses it with the rule `shape_exp` of `src/mapparser.y`. On a layer it replaces each
shape `msLayerNextShape` reads, before classing (`msGeomTransformShape`). On a style it gives the
geometry the style draws (`msDrawTransformedShape`).

Today the IR holds a style's expression as MapServer wrote it, `GeomTransform::Expression(String)`
(`crates/style-ir/src/model.rs`), only so that the engine can report it as not drawn. The style
document so carries MapServer syntax. The glossary's *Style IR* is "free of MapServer syntax", and
its *Core* knows no MapServer syntax. No ADR admits a foreign text in the IR. The layer's
`GEOMTRANSFORM` is not carried at all: the frontend warns `unsupported-keyword` and draws the
shapes as read (`docs/backlog/layer-geomtransform.md`).

The maintainer decided on 2026-11-10 to carry the layer's transform. The engine ports `generalize`,
`densify`, `outer`, `inner`, and `smoothsia`, which need no geometry library, and draws nothing of
a layer of any other function, with a warning. Commit 44cc731 built it as text in the IR, parsed
in the core (`crates/symbolize/src/layer_transform.rs` and its `parse.rs`). The review of
`feat/stable-2026-11-12` found it at odds with the specification and
[ADR-0015](0015-expressions-as-cql2.md). A syntax error passed the load unreported. The parser
also counted no level for a chain of `+ - * / %`, so a long chain overflowed the stack (Q-9). The
maintainer took it out again in 7f36bf6. On 2026-11-13 the maintainer chose a typed tree of the
transform's functions in the IR, which the frontend translates MapServer's expressions into
(`docs/plan/revision-2026-11-13/01-geom-transform-expression-in-ir.md`).

**The grammar at the pin.** `shape_exp` is `[shape]`, parentheses, or one of `buffer(s, n)`,
`inner(s)`, `outer(s)`, `centerline(s)`, `difference(s, s)`, `densify(s, n)`, `simplify(s, n)`,
`simplifypt(s, n)`, `generalize(s, n)`, `smoothsia(s[, n[, n[, text]]])`, and
`javascript(s, text)`. A number `n` is the rule `math_exp`: a number token, `[map_cellsize]`
(which `msGeomTransformShape` binds to the cellsize in the layer's units), parentheses, `+ - * / %
^`, unary `-`, `length`, `area`, and `round`. Three of its quirks bear on the tree:

- unary `-` is `{ $$ = $2; }`, so it leaves its operand un-negated, while the lexer reads `-5` as
  one number token;
- `%` is `(int)$1 % (int)$3`, C's cast ([ADR-0036](0036-c-ports-in-style-ir.md));
- `/` by zero is a parse error, `yyerror(p, "Division by zero.")`, so the transform fails.

`smoothsia`'s text is its preprocessing; the port read `all` and `angle`, without regard to case,
and any other text as none.

**What msautotest uses** (`target/msautotest`, rel-8-6-6 at 7515cccf), 15 expressions:

| Where | Expression | Under this decision |
|---|---|---|
| layer, `misc/centerline.map` | `outer([shape])`, `inner([shape])` | drawn |
| layer | `centerline([shape])` ×2, `centerline(densify([shape], 5))`, `centerline(densify([shape],-5))`, `smoothsia(centerline(densify([shape], 5)))` | not drawn: `centerline` |
| layer, `renderers/geomtransform-vertex.map` | `simplify([shape],25)` | not drawn: `simplify` |
| style, `renderers/geomtransform-vertex.map` | `simplify([shape],10)`, `simplifypt([shape],50)`, `generalize([shape],50)` | style expression, not drawn (today's rule) |
| style, `renderers/polygon-as-line-offset.map` | `buffer([shape], 1)`, `buffer([shape], -1)` | style expression, not drawn |
| style, `renderers/labelpnt.map` | `labelpoint([shape])` | no function of `shape_exp`: a syntax error |

No case uses arithmetic, `[map_cellsize]`, an attribute binding, `difference`, `javascript`, or a
text argument. The other 140 `GEOMTRANSFORM` lines are names (`labelpoly`, `start`, …), which the
IR already holds.

**Forces.** The core must not know MapServer syntax (*Constraints*). The IR round-trips through
YAML and JSON (G-3). Expressions nest at most 24 levels (Q-9), as
[ADR-0015](0015-expressions-as-cql2.md) bounds its tree with `MAX_EXPRESSION_DEPTH`. The document is
1.0 and pre-release: its schema file is regenerated with the IR and the version stays
([ADR-0014](0014-style-document-versions.md), *Freeze*).

## Decision

We will hold a `GEOMTRANSFORM` expression in the style IR as a typed tree of its shape functions.
Only the mapfile frontend translates it from MapServer's text.

- **One type, two places.** The tree is one type of `crates/style-ir`. A layer holds it in the
  optional field `geom_transform`. A class's style holds it in `GeomTransform::Expression`, which
  holds no `String`.
- **Shape.** The tree is written in ADR-0015's CQL2-JSON shape, `{"op": name, "args": [...]}`. Its
  nodes are:
  - `shape`, without arguments, the feature's shape, MapServer's `[shape]`;
  - `generalize` and `densify`, each of a shape and a number;
  - `outer` and `inner`, each of a shape;
  - `smoothsia`, of a shape and up to three further arguments: a size, a number of iterations,
    and a preprocessing of `all` or `angle`.
- **Numbers.** A number argument is a JSON number, `map_cellsize` without arguments, or `+`, `-`,
  `*`, `/`, `%`, or `^` of two number arguments.
  - The frontend must fold every operation without `map_cellsize` to one JSON number at load.
  - The frontend must fold `[data_cellsize]` to the number 0, as `mapgeomtransform.c` binds it on
    every layer but a contour layer, which Forager does not carry.
  - The IR has no negation. The frontend must translate MapServer's unary `-` to its operand, as
    `mapparser.y` does.
  - The frontend must translate a text `smoothsia` preprocessing other than `all` or `angle`, read
    without regard to case, to no preprocessing.
- **Not drawn.** A transform the engine does not draw is the whole tree `{"not_drawn": reason}`.
  The reason is one name of a closed list:
  - each function of `shape_exp` and `math_exp` the tree lacks: `buffer`, `centerline`,
    `difference`, `simplify`, `simplifypt`, `javascript`, `length`, `area`, `round`;
  - `binding`, for a property's value where a number or a text is due;
  - `text`, for a computed `smoothsia` preprocessing;
  - `fromtext`, for a shape `fromtext` reads from a text;
  - `literal`, for an operation of literals that folds to no finite JSON number, a `%` by 0 among
    them;
  - `syntax`, for an expression `shape_exp` does not parse, a `/` by 0 among them.

  A syntax error takes precedence: MapServer refuses the whole expression, so an expression with a
  syntax error is `syntax`. Otherwise the frontend must name the first such construct in the
  expression's reading order. The tree must hold no text of the mapfile.
- **Who parses.** Only `crates/mapfile` may parse a `GEOMTRANSFORM` expression. The core must read
  the tree and never MapServer's text.
- **Diagnostics.** The engine must warn `geomtransform-not-drawn` for a `not_drawn` tree, naming
  its reason. The frontend must warn at load only of a syntax error, with its place.
- **Bounds.** The tree must nest at most `MAX_EXPRESSION_DEPTH`, 24 levels, counted as ADR-0015
  counts an expression's depth.
  - The frontend's parser must count each operator of a chain as one level of the tree it builds.
  - It must refuse a deeper tree before it recurses deeper, without overflowing the stack.
  - A deeper tree is the load error `expression-too-deep`, from a mapfile and from a style
    document alike (Q-9).
- **Versions.** The document stays at 1.0, and `schema/style-document-1.0.json` is regenerated in
  the change that implements this ADR. A function added to the tree after the first release must
  be a major version. A reader of an older minor unsets an unknown function, and so draws the
  shapes untransformed. It must read a `not_drawn` reason it does not know as not drawn, with a
  warning naming its path. A reason added in a minor version so fails closed.

**Out of scope:**

- the evaluation of the tree and each function's semantics, which `docs/semantics/` writes down
  from the C, `[map_cellsize]` and the layer's `UNITS` among them;
- drawing a class style's expression: the engine keeps drawing none, as today;
- the bounds on the points a transform adds and visits, which Q-9 does not yet name;
- a label's style of an expression, which keeps `LabelTransform` and is left out with its label as
  today;
- porting any function of the not-drawn list, `buffer` and `centerline` among them.

## Alternatives considered

- **Keep MapServer's text in the IR, admitted by an ADR** — no translation, and a later port
  needs no frontend change. The core would parse MapServer syntax, against the *Constraints* and
  the glossary's *Style IR*. A syntax error would pass the load unseen. 44cc731 showed both, with
  a stack overflow besides.
- **A unit variant, `Unsupported`, with the text only in the load's diagnostic** — smallest for
  today's class styles, which draw no expression. It cannot carry the five ported functions of a
  layer, which the maintainer decided to draw.
- **The style or the layer left out at load with its warning** — what a label's style of an
  expression or of a transform other than `labelpnt`, `labelpoly`, or `labelcenter` gets. A style
  document could then not say that a layer draws nothing. A layer of `centerline` would draw its
  untransformed shapes, where MapServer draws only centerlines.
- **The not-drawn function kept in place in the tree, with its arguments** — `buffer(s, 1)` as a
  node. It types arguments the engine never reads. `difference` needs a second shape, and
  `javascript` holds a script text.
- **A chain instead of a tree**, each function applied to the result of the one before — no
  recursion in the shape, and a length bound instead of a depth. Its shape differs from ADR-0015's
  tree, so two depth rules and two lenient readers would hold. `difference` of two shapes would
  not fit it.
- **The numbers as ADR-0015 expressions, evaluated by `expr`** — one type for every number. It
  admits properties, texts, and conditions the transform refuses, and `expr` would need
  `map_cellsize` from the render.
- **Literal numbers only, `[map_cellsize]` not drawn** — smallest by msautotest, which uses
  neither. The decision of 2026-11-10 ported `[map_cellsize]`, as the backlog entry states, and
  a tolerance in pixels needs it.
- **Supersede ADR-0015** — not needed. ADR-0015 governs expressions over a feature's properties,
  and it leaves geometry functions out of scope. This tree is a sibling: it borrows ADR-0015's
  JSON shape and its depth bound, and it changes none of ADR-0015's clauses.

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapparser.y`: the rules `shape_exp` and `math_exp` and their
  precedences, read 2026-11-13 from a copy of the pin. `msGeomTransformShape`,
  `src/mapgeomtransform.c`, and `msLayerNextShape`, `src/maplayer.c:444-449`, as commit 44cc731
  cites them.
- The msautotest suite at 7515cccf: every `GEOMTRANSFORM` line of its mapfiles, counted
  2026-11-13 (table above).
- Commit 44cc731: the earlier port, its parser, and its tests derived from the C. Commit 7f36bf6,
  which removed it.
- [ADR-0015](0015-expressions-as-cql2.md), [ADR-0014](0014-style-document-versions.md),
  [ADR-0008](0008-ports-and-adapters-crates.md), [ADR-0036](0036-c-ports-in-style-ir.md).
- `docs/backlog/layer-geomtransform.md`, `docs/plan/revision-2026-11-13/01-…` and `12-…`,
  `docs/semantics/style-symbol.md`.

## Consequences

- Positive: the style document holds no MapServer syntax, and the core parses none. A syntax error
  is reported at load. The bound of Q-9 holds on the parser and on the document reader alike. The
  layer's `GEOMTRANSFORM` comes back with the five functions of the 2026-11-10 decision, and
  `misc/centerline.map`'s `outer` and `inner` layers can pass.
- Positive: a style document may write a transform no mapfile wrote, and a document reader outside
  Forager reads it as CQL2-JSON custom functions.
- Negative / trade-offs: a later port of `buffer` or `centerline` changes the IR, the frontend,
  and the schema, not only the engine. The frontend grows a parser of `shape_exp` and `math_exp`.
  A `not_drawn` reason keeps less than the text did: the detail stays in the load's diagnostic.
- Follow-ups:
  - the semantics page of the layer's transform, with `[map_cellsize]` and the layer's `UNITS`;
  - Q-9's bounds on the points a transform adds and visits, a change to the specification;
  - a glossary entry for the transform tree.

## Enforcement

- Tests of the transform tree in `crates/style-ir` cite this ADR with `Verifies:`. They read and
  write the CQL2-JSON shape of each node, number, and `not_drawn` reason.
- A test of `crates/style-ir/src/document.rs` cites this ADR and Q-9 with `Verifies:`. It refuses
  a document whose transform tree nests past `MAX_EXPRESSION_DEPTH`.
- A test of `crates/style-ir/tests/schema.rs` cites this ADR with `Verifies:`. It walks the schema
  of the transform tree and of `GeomTransform`. It fails on any string that is not a value of a
  closed enum. This decides by type that the tree holds no mapfile text, which no text search of
  the code decides.
- Tests of the mapfile frontend cite this ADR with `Verifies:`. They translate each expression of
  the table above to its tree or its reason. They cover:
  - the folding of literal arithmetic, and `map_cellsize` kept;
  - unary `-` left un-negated;
  - a chain of 25 `+` refused as `expression-too-deep`, without a stack overflow (Q-9);
  - a syntax error warned with its place.
- A test of `crates/engine` cites this ADR with `Verifies:`. A layer of a `not_drawn` tree draws
  nothing and warns `geomtransform-not-drawn` with its reason.

**Not mechanically decidable:** that no core crate parses MapServer's text by another route, such
as a string field added elsewhere. The schema test covers the transform tree only. Review catches
the rest.
