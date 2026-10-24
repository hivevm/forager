# Ticket: Carry CLASSITEM, class EXPRESSION, and layer FILTER into the IR as CQL2 trees

## What to build

The tracer bullet through expressions: `forager read` on a mapfile with `CLASSITEM`, a class
`EXPRESSION` of each MapServer kind — a string, a string with `i`, a regular expression, a list,
and a logical expression — and a layer `FILTER` prints the IR with each expression as the
CQL2-JSON tree of ADR-0019, typed with `to_number` and `to_string`, and the document reads back
equal. Neither keyword is reported as unsupported any more. A construct outside the subset is a
warning, and its class or filter is kept and never matches. The tree's nesting is limited (Q-9).
Writes `docs/semantics/` pages for `CLASSITEM`, `EXPRESSION`, and `FILTER`.

## Acceptance criteria

- [ ] Given `CLASSITEM "type"` and `EXPRESSION "road"`, when the mapfile loads, then the class
  holds `{"op":"=","args":[{"property":"type"},"road"]}`.
- [ ] Given `EXPRESSION "Road"i`, `EXPRESSION /^r.*d$/i`, and `EXPRESSION {a,b,c}`, when the
  mapfile loads, then they hold `=` over `casei`, `regexp_like` with the flag `i` and the pattern
  verbatim, and `in` with the three values.
- [ ] Given `EXPRESSION ([pop] >= 1000 AND "[name]" =* "x")`, when it loads, then `pop` is wrapped
  in `to_number`, `name` in `to_string`, and the comparison is `>=` inside `and`.
- [ ] Given `FILTER ('[a]' in 'A,B,C')` and ``FILTER (`[t]` > `2010-12-01`)``, when the layer
  loads, then it holds `in` with the list split at load and a temporal predicate.
- [ ] Given an expression on `[shape]`, when it loads, then a warning names the construct and the
  class never matches.
- [ ] Given an expression nested past the limit, from a mapfile or from a style document, when it
  is read, then the load reports the limit (citing Q-9).
- [ ] Given each expression above, when the document is written as YAML and as JSON and read
  back, then it is equal (citing ADR-0019).

## Read first

- [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0014](../../adr/0014-style-document-serde.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); the spec next to this ticket.
- Golden path `forager read` (`docs/ARCHITECTURE.md`); the frontend's tests at its `load` seam.
- MapServer `rel-8-6-6` `src/mapparser.y` and `msEvalExpression` in `src/maputil.c` for the
  semantics pages.

## Blocked by

- None (can start immediately); ADR-0019 is accepted before it lands.

## Mode

with the human: it changes the style document format, which the harness keeps with the
maintainer ([ADR-0007](../../adr/0007-harness.md)).
