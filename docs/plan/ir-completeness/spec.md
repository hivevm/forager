# Spec: The styling core in the style IR

## Problem

A cartographer who loads a mapfile gets back a style IR that holds a map's extent, size, and
layers with their classes and four style properties — colour, outline colour, width, and size.
Everything else that decides how the map looks is reported as "not yet supported" and dropped:
which features a class draws (`CLASSITEM`, `EXPRESSION`), which features a layer reads
(`FILTER`), symbols and symbol sets, labels and font sets, the layer's projection, and the scales
at which layers and classes show. Of the 503 mapfiles of the msautotest suite, 453 load; the rest
fail on attribute bindings such as `SIZE [VAL]`, a colour of −255, two `INCLUDE "../…"`, and
EPSG codes outside the built-in resolver. A style document written as JSON does not even read
back with the same numbers. The IR cannot yet be what the engine renders from.

## Solution

Every mapfile of the msautotest suite, except the include fragments and the malformed files named
by the test, loads without an error into a style IR that carries the styling core, and that IR
survives the round trip through YAML and JSON unchanged. The IR carries the parts that render:
classes with their expressions and scale ranges; styles complete, attribute bindings and symbol
references included; symbols and symbol sets; labels and font sets; and the layer's filter,
class item, projection, and scale ranges. MapServer's quirks are resolved at import, so the IR
holds values that mean what they say. Each parameter the IR carries has a short semantics page
that says what it means and which MapServer behaviour it reproduces. What is not carried over
stays a "not yet supported" warning.

## Existing behaviour

- The frontend translates `MAP` (name, extent, size, image colour, projection), `LAYER` (name,
  type, status, connection type, connection, data), `CLASS` (name), and `STYLE` (colour, outline
  colour, width, size), and reports every other keyword as unsupported or discarded. Pinned by
  the frontend's tests at its `load` seam and by `forager read`'s CLI tests.
- `INCLUDE` splicing, its bounds (Q-9), containment (Q-8), and the diagnostics cap stay as they
  are; their tests stay green unchanged. The CLI keeps the mapfile's directory as the base
  context by default.
- `init=` resolution stays as ADR-0018 decides it; no EPSG data enters the product.
- A colour of −1 still means "no colour".
- The engine keeps rendering from the IR fields it reads today; new IR fields it does not read
  yet change no pixel, so the engine's and the browser's render tests stay as they are.
- The style document's format version stays `1.0` through this spec; the IR grows in place.
  Versioning, migration, and unknown fields are the following spec's (G-4), and no document
  written by an earlier build has to be read back.

## Goals served

- **G-1** — met: the msautotest suite loads without an error, by a test over the pinned corpus.
- **G-3** — met: the round trip mapfile → IR → YAML → JSON → IR yields an equal IR, by a test
  over the same corpus and by the fuzz target.
- **G-2** — kept: the CLI prints the larger IR and its diagnostics.
- **G-7** — prepared: filters and class expressions are CQL2 trees (ADR-0019), which pushdown
  will take; evaluation is not part of this spec.
- **Q-8**, **Q-9** — extended: `SYMBOLSET`, `FONTSET`, and pixmap paths resolve inside the base
  context; expression nesting is bounded.
- **Q-4** — prepared: the quirks resolved at import are the ones rendering fidelity depends on.

## User stories

1. As a cartographer, I want every msautotest mapfile to load without an error, so that I can
   trust a mapfile of mine that MapServer accepts to load in Forager too.
2. As a cartographer, I want a class's `EXPRESSION` and a layer's `FILTER` carried into the IR,
   so that the right features get the right style.
3. As a cartographer, I want `SIZE [attr]`, `WIDTH [attr]`, `COLOR [attr]`, and `ANGLE [attr]`
   carried as bindings, so that data-driven styling survives the import.
4. As a cartographer, I want styles carried completely — symbol, angle, offset, gap, pattern,
   line caps and joins, opacity, and the size and width bounds — so that a line or a marker looks
   as it does in MapServer.
5. As a cartographer, I want the symbols of a `SYMBOLSET` file and inline `SYMBOL` blocks in the
   IR, so that a style's symbol reference resolves.
6. As a cartographer, I want labels with their text expressions, fonts from the `FONTSET`, and
   placement parameters in the IR, so that the engine can label later without going back to the
   mapfile.
7. As a cartographer, I want a layer's own `PROJECTION` and the scale ranges of layers and
   classes in the IR, so that a layer shows in the right place at the right scales.
8. As a cartographer, I want MapServer's quirks — a vector symbol's size being its height, dash
   lengths scaling with the width, `OFFSET x -99`, `SYMBOLSCALEDENOM` with `MINSIZE` and
   `MAXSIZE`, 72 versus 96 dpi, `SIZEUNITS` — resolved when the mapfile is read, so that the IR
   says what is drawn and not what MapServer would make of it.
9. As an operator, I want `forager read --base <dir>` to allow `INCLUDE "../…"` inside a
   directory I name, so that mapfiles that share includes one level up load without opening the
   whole file system.
10. As a developer embedding Forager, I want a style document written as JSON or YAML to read
    back to exactly the same IR, so that I can store and ship documents instead of mapfiles.
11. As a developer, I want a short semantics page for every mapfile parameter the IR carries, so
    that I know what a field means without reading MapServer's C code.
12. As a maintainer, I want CI to load and round-trip the whole msautotest suite at a pinned
    MapServer release, so that a change that breaks a real mapfile fails before it merges.

## Implementation decisions

- **Expressions** are the CQL2 tree of ADR-0019, in the IR crate; the frontend translates the four
  MapServer kinds into it, typing each attribute explicitly. A construct outside the subset is a
  warning, and the class or filter that carries it is kept and never matches.
- **Bound values.** A style property that MapServer allows to be bound holds a value, a
  binding of a property read with MapServer's binding rules, or an expression of the same tree
  where MapServer takes one, so one evaluator serves filters, classes, labels, and expression
  bindings.
- **Colours** accept −255…255 per channel as MapServer's `loadColor` does, and fail past it; −1
  stays "no colour". What another negative channel means is what MapServer's renderer makes of
  it, determined by the ticket and resolved at import, so the IR holds only 0…255.
- **Symbols** live once in the map, keyed by name, from `SYMBOLSET` files read through the
  resource loader and from inline `SYMBOL` blocks; a style references a symbol by name or index,
  resolved at import. A pixmap symbol's image path is a resource path under Q-8; its bytes are not
  read at load.
- **Fonts** are the `FONTSET` file's name-to-path table, read through the resource loader; a label
  names its font. Font files are not read at load.
- **Labels** belong to their class; `TEXT` is a string expression, `LABELITEM` a property.
- **Units and scale.** The IR holds lengths in pixels of the output image: the frontend scales
  them by `RESOLUTION` over `DEFRESOLUTION` (both 72 dpi unless the mapfile sets them, as in
  MapServer), so the IR does not carry the two; `SIZEUNITS` other than
  pixels and `SYMBOLSCALEDENOM` become a scale factor the engine applies, together with the
  clamping bounds, without the IR repeating MapServer's arithmetic. The semantics page of each
  quirk states the formula and the MapServer source it follows.
- **Projection** of a layer resolves like the map's (ADR-0018); reprojection is not part of this
  spec.
- **Base context.** The CLI takes `--base <dir>`; the mapfile must lie inside it, and `..` is
  allowed as long as the path stays inside it (Q-8 unchanged in substance: the base context is
  what the host gives).
- **Expression nesting** is limited, and exceeding it is a diagnostic (Q-9); the number is set in
  the ticket that adds the tree and written into Q-9 with the other limits.
- **Semantics pages** go under `docs/semantics/`, one per mapfile parameter the IR carries, each
  written by the ticket that carries the parameter: what it means, the MapServer behaviour it
  reproduces with its source, and how the frontend resolves its quirks.

## Testing decisions

- The frontend's `load` is the seam for every translation, as the existing frontend tests do,
  with inline mapfiles and an in-memory loader; the CLI tests stay the seam for `--base`.
- The style document's own tests are the seam for the CQL2-JSON shape and the exact number round
  trip.
- **G-1** is a test over the msautotest suite of MapServer `rel-8-6-6`, fetched by a pinned
  sparse checkout and cached in CI, never vendored; the EPSG codes outside the built-in resolver
  come from a fixture table handed in through the host table of ADR-0018, in the test only. The
  files it skips — include fragments, malformed and binary test data, a file whose include lies
  outside the checkout — are named in the test with a reason each.
- **G-3** is a test over the same corpus, and a property of the `mapfile` fuzz target.
- The survey script under `.scratch/` is the prior art for the checkout.

## Out of scope

- Evaluating expressions, pushdown, and the regular-expression engine (G-7).
- Rendering anything new: symbols, labels, scale ranges, and bindings reach the IR, not the image.
- Reprojection, and layer projections beyond resolving them.
- Versioning, migration, unknown fields, and the JSON Schema of the style document (G-4), the next
  spec.
- `LEGEND`, `SCALEBAR`, `OUTPUTFORMAT`, `METADATA`, `WEB`, `QUERYMAP`, and the other keywords
  outside the styling core, which stay warnings.
- Geometry functions and spatial predicates in expressions; CQL2-Text.
- G-5 and Q-9's feature and file limits, which stay tickets of the design revision.

## ADR impact

- Binding: ADR-0009 (crates and edges; the IR stays free of workspace dependencies), ADR-0012
  (supply chain, for any feature or crate added), ADR-0014 (the style document through serde),
  ADR-0018 (projection resolution), ADR-0007 (the style document format and resource paths stay
  with the maintainer).
- New: ADR-0019 (expressions as a CQL2 tree), proposed before this spec, to be accepted before its
  first ticket lands.
- `docs/semantics/` is a reference document under the "every other document" clause of ADR-0001:
  it describes, and holds no rule.
