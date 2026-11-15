# ADR-draft-transform-tree-with-simplify-and-buffer: A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, `simplify`, `simplifypt`, and `buffer` among its nodes

- **Status:** 🟡 proposed
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.1.json`
- **Supersedes:** [ADR-0036](0036-typed-geomtransform.md)

## Context

[ADR-0036](0036-typed-geomtransform.md) holds a `GEOMTRANSFORM` expression as a typed tree of its
shape functions. Its *Context* gives the grammar at the pin, the expressions msautotest uses, and
the forces; they still hold. Its *Decision* lists the tree's nodes: `shape`, `generalize`,
`densify`, `outer`, `inner`, and `smoothsia`. It names `buffer`, `simplify`, and `simplifypt` as
reasons a tree is not drawn. Its *Out of scope* leaves the port of any function of that list open.

The fidelity plan ports the three functions (tickets 11 and 12). The code that computes them is
[ADR-draft-geometry-ported-from-jts](draft-geometry-ported-from-jts.md). The tree must then hold
them as nodes with their arguments. That changes ADR-0036's list of nodes and its closed list of
reasons, so this ADR supersedes it. It restates every clause of ADR-0036 in its wording, but
these:

- *Shape*: the nodes `simplify`, `simplifypt`, and `buffer` are added;
- *Order of landing*: a new entry, each node landing with the change that draws it;
- *Numbers*: a new item, a negative tolerance or distance kept as written;
- *Not drawn*: `buffer`, `simplify`, and `simplifypt` stay reasons only until their nodes land;
- *Versions*: two new items, the regenerated file of 1.1 and the version move left to the
  maintainer;
- *Out of scope*: the code of the three functions added, after ADR-0036's first item; the
  bounds of a transform's points narrowed to those Q-9 does not name, since Q-9 now names a
  layer's; and the port of a not-drawn function no longer giving `buffer` as an example.

**The grammar of the three** (`src/mapparser.y` at `rel-8-6-6`):

- `buffer(s, n)` (`:758`) calls `msGEOSBuffer(s, n)`, a distance in the shape's unit, which may
  be negative;
- `simplify(s, n)` (`:842`) calls `msGEOSSimplify(s, n)`, a tolerance in the shape's unit;
- `simplifypt(s, n)` (`:856`) calls `msGEOSTopologyPreservingSimplify(s, n)`, a tolerance too.

Each `n` is the rule `math_exp`, as for `generalize` and `densify`. MapServer fixes the 30
segments a quadrant of `buffer` in the C; no argument carries them.

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
  - `simplify` and `simplifypt`, each of a shape and a number, the tolerance;
  - `buffer`, of a shape and a number, the distance;
  - `outer` and `inner`, each of a shape;
  - `smoothsia`, of a shape and up to three further arguments: a size, a number of iterations,
    and a preprocessing of `all` or `angle`.
- **Order of landing.** Each of `simplify`, `simplifypt`, and `buffer` must enter the tree in the
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

**Out of scope:**

- the evaluation of the tree and each function's semantics, which `docs/semantics/` writes down
  from the C, `[map_cellsize]` and the layer's `UNITS` among them;
- the code that computes `simplify`, `simplifypt`, and `buffer`, which
  [ADR-draft-geometry-ported-from-jts](draft-geometry-ported-from-jts.md) decides;
- drawing a class style's expression: the engine keeps drawing none, as today;
- the bounds on the points a transform adds and visits beyond those Q-9 names;
- a label's style of an expression, which keeps `LabelTransform` and is left out with its label as
  today;
- porting any function of the not-drawn list, `centerline` among them.

## Alternatives considered

- **Keep ADR-0036 and add the nodes under it** — no new record. ADR-0036's *Decision* lists the
  nodes and the closed list of reasons, so adding a node changes an accepted decision. An accepted
  ADR changes only by supersession ([`AGENTS.md` §3](../../AGENTS.md#3-adr-rules), rule 5).
- **One node for both simplifications, with a flag for topology** — one name fewer. MapServer's
  grammar has two functions, and the tree mirrors `shape_exp` function by function. A flag would
  be a field no CQL2-JSON function has.
- **`buffer` with its segments a quadrant as a third argument** — open for a style document that
  wants fewer. No mapfile can write it, and MapServer fixes 30. It can come later as an optional
  argument, which ADR-0014's version rule allows as a minor.
- **All three nodes at once, before the engine draws `buffer`** — one schema change instead of
  two. The tree would hold a node the engine reads as not drawn. ADR-0036 rejected nodes the engine
  never reads, and a `not_drawn` reason says the same with less.
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

## Sources / Prior art

- MapServer `rel-8-6-6`, `src/mapparser.y` (`:758`, `:842`, `:856`) and `src/mapgeos.c`
  (`:899`, `:922`, `:946`), read 2026-11-15 from `target/ms-src`.
- The msautotest suite at 7515cccf: `renderers/geomtransform-vertex.map` and
  `renderers/polygon-as-line-offset.map`, read 2026-11-15.
- [ADR-0036](0036-typed-geomtransform.md), whose *Context* and *Sources* this ADR keeps.
- [ADR-0014](0014-style-document-versions.md), [ADR-0015](0015-expressions-as-cql2.md),
  [ADR-0008](0008-ports-and-adapters-crates.md), [ADR-0017](0017-c-ports-in-style-ir.md).
- `docs/plan/fidelity/11-simplify-through-a-geometry-library.md` and
  `docs/plan/fidelity/12-geomtransform-buffer.md`.

## Consequences

- Positive: a layer of `simplify` or `simplifypt` draws, and the case of `vertex2` can pass. A
  style document may write the three functions as CQL2-JSON custom functions.
- Positive: every clause of ADR-0036 but those the *Context* lists keeps its wording, so the
  frontend's tests and the engine's rules of a `not_drawn` tree stand unchanged.
- Negative / trade-offs: a document of 1.1 written before a node lands, with that function's
  reason, fails to load after it. No release holds 1.1, so only local files are affected.
- Negative / trade-offs: the schema changes with each change that lands a node: two changes, or
  three if `simplifypt` splits from `simplify`, as ticket 11's *Mode* allows.
- Follow-ups:
  - the semantics page of the layer's transform, with `[map_cellsize]`, the layer's `UNITS`, and
    GEOS's failures;
  - Q-9's bounds on the points a style's expression adds and visits, a change to the
    specification, if the engine draws a style's expression.

## Enforcement

- Tests of the transform tree in `crates/style-ir` cite this ADR with `Verifies:`. They read and
  write the CQL2-JSON shape of each node, number, and `not_drawn` reason, `simplify`,
  `simplifypt`, and `buffer` among the nodes once they land.
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
  - a chain of 25 `+` refused as `expression-too-deep`, without a stack overflow (Q-9);
  - a syntax error warned with its place.
- A test of `crates/engine` cites this ADR with `Verifies:`. A layer of a `not_drawn` tree draws
  nothing and warns `geomtransform-not-drawn` with its reason.
- The tests that cite [ADR-0036](0036-typed-geomtransform.md) move their `Verifies:` to this ADR in
  the change that implements it.

**Not mechanically decidable:** that no core crate parses MapServer's text by another route, such
as a string field added elsewhere. The schema test covers the transform tree only. Review catches
the rest.
