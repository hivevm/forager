# Ticket: Check expression nesting once per map, after translation

## What to build

The frontend checks the expression nesting of Q-9 at each entry point of
`crates/mapfile/src/expression.rs` (`number_value`, `text`, `logical`) beside a parser guard at four
times the bound; `style-ir` checks it again in `StyleDocument::bounded`, which is private, and in
`lenient.rs`; `expr` checks it at evaluation. A new entry point of the frontend that forgets it
yields an IR the document reader refuses. Make the map-level check of `style-ir` public and run it
once after `translate`, keeping the parser's recursion guard and a span for the diagnostic.

## Acceptance criteria

- [ ] Given a mapfile with an expression 25 levels deep in each place the frontend reads one
  (filter, class expression, text, a bound number), when it is loaded, then each gives the
  diagnostic of the bound with its span.
- [ ] Given the frontend's entry points, when they are read, then none checks the depth itself.

## Read first

- `crates/mapfile/src/expression.rs`; `bounded` in `crates/style-ir/src/document.rs`;
  `crates/style-ir/src/lenient.rs`; the Q-9 nesting test.

## Blocked by

- None (can start immediately)

## Mode

with the human: it moves where a resource limit of Q-9 is held, which ADR-0007 keeps with the
maintainer.
