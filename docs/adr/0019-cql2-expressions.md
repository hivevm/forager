# ADR-0019: Expressions in the style IR are a lean CQL2 tree of Forager's own types, serialized as CQL2-JSON, with MapServer's typing made explicit at import

- **Status:** ⚪ superseded by ADR-0030
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir`, the translation of expressions in `crates/mapfile`, and every crate that evaluates an expression of the style IR

## Context

The strategy translates mapfiles at load time into a style IR "whose expressions are CQL2"
(*Strategy*); only the frontend knows MapServer syntax (*Constraints*). Phase 1 carries the
styling core into the IR — class `EXPRESSION`, layer `FILTER`, label `TEXT`, and attribute
bindings such as `SIZE [attr]` — so the IR needs an expression type before any of them land. It
must round-trip through YAML and JSON (G-3), evaluate identically with and without pushdown
(G-7), build for `wasm32-unknown-unknown` without C (Q-1), stay small in the bundle (Q-5), and
bound its nesting (Q-9).

MapServer 8.6.6 (`src/mapparser.y`) parses four kinds of expression: a string compared to the
`CLASSITEM` (`"value"`, `"value"i`), a POSIX extended regular expression on it (`/re/`, `/re/i`),
a comma list (`{a,b,c}`), and a logical expression in parentheses. The last has typed
sub-grammars — logical, number, string, time, shape — and **the syntax decides the type**: `[a]`
is a number, read with C's `atof` (a non-numeric value is 0), and `"[a]"` is a string. Its
operators are comparison (`=`, `!=`, `<`, …, and the word forms `eq`, `ne`, …), case-insensitive
equality `=*`, regex match `~` and `~*`, `IN` against a comma-separated string, `AND`, `OR`,
`NOT`, arithmetic `+ - * / % ^`, string concatenation `+`, string functions (`tostring`,
`commify`, `upper`, `lower`, `initcap`, `firstcap`, `length`, `round`), time literals in
backticks, and geometry functions and predicates on `[shape]`. In the msautotest suite, 294
`EXPRESSION` and `FILTER` lines use 180 logical expressions, 82 strings, 25 regular expressions
and 7 lists; the logical ones use comparison, `AND`/`OR`/`NOT`, `=*`, `~`/`~*`, `IN`, four time
comparisons, three string functions, and no `[shape]` at all. Nine `TEXT` expressions use
`tostring` and concatenation.

CQL2 (OGC 21-065r2) types a comparison by its operands' values, not by syntax. It has
comparison, `like`, `between`, `in`, `isNull`, `and`/`or`/`not`, arithmetic including `^` and `%`,
`casei`, spatial `s_*` and temporal `t_*` predicates, and custom functions, which CQL2-JSON
writes as `{"op": <name>, "args": [...]}` with any name that is not a reserved operator. It has
no regular expression, no concatenation, and none of MapServer's string functions.

The maintainer decided the AST is the project's own lean, serializable types in `style-ir`
rather than a dependency, and chose the encoding, the extension mechanism, and the typing below.

## Decision

We will represent every expression of the style IR as a tree of Forager's own serde types in
`style-ir` that covers the subset of CQL2 MapServer's expressions need, serialized in the shape of
CQL2-JSON, with what CQL2 lacks written as CQL2 custom functions and MapServer's syntax-decided
types made explicit by the frontend as conversion functions around each attribute it reads.

- **Shape.** A property is `{"property": name}`, a time is `{"timestamp": …}` or `{"date": …}`,
  strings, numbers, and booleans are JSON literals, and every operator or function is
  `{"op": name, "args": [...]}`. The operators are CQL2's: `=`, `<>`, `<`, `>`, `<=`, `>=`,
  `in`, `and`, `or`, `not`, `+`, `-`, `*`, `/`, `%`, `^`, `casei`, and the temporal predicates.
- **Typing.** An attribute read as a number is wrapped in `to_number`, with `atof` semantics, one
  read as a string in `to_string`, and one read as a time in `to_timestamp`, with
  `msParseTime` semantics. The IR therefore means the same whatever type a feature source
  delivers.
- **Functions** for what CQL2 lacks: `regexp_like(value, pattern, flags)` with the pattern kept
  verbatim as POSIX extended syntax and the flag `i`; `concat`; and MapServer's string functions
  under their own lower-case names. Each function's semantics is MapServer's and is written down
  where the IR's semantics are specified.
- **Import.** The frontend translates each kind into this tree: a string, regular expression, or
  list against the `CLASSITEM`, or the `FILTERITEM` for a layer's filter, becomes `=`,
  `regexp_like`, or `in` on that property, `"value"i` and `=*` become `=` over `casei`, `IN`
  against a literal string becomes `in` with the list split at the commas at load time, a chain
  of `AND` or of `OR` becomes one `and` or `or` over all its operands, and time comparisons
  become temporal predicates. A construct outside the subset — geometry functions and
  predicates, `IN` against a computed string — and an expression MapServer cannot parse are a
  warning, and the class or filter that carries it is kept with the expression `false`, so that
  it never matches, as MapServer never matches an expression it cannot parse.
- **Bounds.** The tree nests at most 24 deep, so that a document holding it stays inside the
  nesting limits of the YAML and JSON readers; it is checked when the tree is read, from a
  mapfile or a style document, and exceeding it is a load error (Q-9).

**Out of scope:** evaluating expressions and choosing the regular-expression engine, which come
with the engine's use of the IR and with pushdown (G-7); CQL2-Text, as input or output; geometry
functions and spatial predicates, which no msautotest case uses; validating documents against the
OGC JSON Schema, which is the style document's schema work (G-4).

## Alternatives considered

- **A CQL2 crate as the AST** — would bring a parser and CQL2-Text, but a dependency the core
  must build for `wasm32` and carry in the bundle (Q-5), with a model wider than MapServer needs
  and none of its functions; the maintainer chose own types.
- **CQL2-Text strings in the document** — easier to read in YAML, but every reader then needs a
  CQL2-Text parser in the core, more surface for Q-3 and Q-9 than a serde tree.
- **Both, Text read and JSON written** — convenient, at the cost of two readers and a document
  that does not round-trip byte for byte.
- **Own operators for regex and case-insensitive match** (`"~"`, `"=*"`) — closer to MapServer,
  but no longer CQL2-JSON; custom functions are CQL2's own extension point.
- **The type on the property** (`{"property": a, "type": "number"}`) — more compact, but not
  CQL2-JSON, and a reader that ignores the field silently changes the meaning.
- **Keep MapServer's expression text in the IR** — no translation now, but the core would have to
  know MapServer syntax, which the constraints forbid, and the strategy's CQL2 would never come.

## Sources / Prior art

- MapServer `rel-8-6-6`: `src/mapparser.y` (the grammar, `IN` against a comma list, `atof` on
  `MS_TOKEN_BINDING_DOUBLE`, `ms_regcomp` with `MS_REG_EXTENDED`), `src/maputil.c`
  (`msEvalExpression`: `MS_STRING`, `MS_LIST`, `MS_REGEX`, the `i` flag as `strcasecmp`).
- OGC 21-065r2, *Common Query Language (CQL2)*, <https://docs.ogc.org/is/21-065r2/21-065r2.html>,
  and its JSON Schema <https://schemas.opengis.net/cql2/1.0/cql2.json> (`functionRef`, `casei`,
  `arithmeticExpression`), read 2026-10-24.
- The msautotest counts above, from `EXPRESSION`, `FILTER`, and `TEXT` lines of the suite at
  `rel-8-6-6`.

## Consequences

- Positive: the core reads a typed, MapServer-free tree; a filter can be handed to a source that
  speaks CQL2 for pushdown; the document is CQL2-JSON a reader outside Forager understands up to
  the custom functions.
- Negative / trade-offs: expressions are verbose in YAML; the custom functions are Forager's, so
  another CQL2 consumer cannot evaluate them; `regexp_like` keeps POSIX syntax that the engine
  must match with a non-POSIX regular-expression crate.
- Follow-ups: the regular-expression engine and its size limit (Q-9); evaluation and pushdown
  (G-7); geometry functions when a case needs them.

## Enforcement

Tests of `style-ir` read and write the CQL2-JSON shape of each operator, function, and literal
and refuse a tree past the nesting limit; tests of the mapfile frontend translate each kind and
operator of the msautotest suite, the typing by `to_number` and `to_string`, and the warning for
a construct outside the subset; all cite this ADR with `Verifies:`.
