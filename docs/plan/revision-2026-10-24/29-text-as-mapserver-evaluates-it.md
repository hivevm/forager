# Ticket: A label's TEXT as msEvalTextExpression evaluates it

## What to build

Found by area 1 of the port review (revision ticket 23, #68), at `rel-8-6-6`:

- **A condition as TEXT** (`TEXT ([a] > 1)`) is the string `"true"` or `"false"` in MapServer
  (`src/mapparser.y:68-80`); the frontend warns. Carrying it needs a conditional in the IR's
  expressions, a change of the style document's format.
- **A `'` before a binding** in a plain TEXT (`TEXT "it's [name]"`): `msEvalTextExpression`
  replaces only the bindings the expression lexer finds, and the `'` opens a string that never
  closes, so `[name]` stays as written; the frontend substitutes it. `[shape]` too stays as
  written in MapServer.

Each is decided with the maintainer: carried as MapServer does, or refused with a diagnostic
that names the difference; neither is drawn differently without a word.

## Acceptance criteria

- [ ] Given each case above, when it loads, then it is read as decided, a test each, and
  `docs/semantics/label.md` says so with the source.

## Read first

- [ADR-0019](../../adr/0019-cql2-expressions.md); `crates/mapfile/src/expression.rs` (`text`);
  `docs/semantics/label.md`, `docs/semantics/expression.md`.
- MapServer `rel-8-6-6`: `msEvalTextExpression` (`src/maputil.c`), `src/mapparser.y`.

## Blocked by

- None (can start immediately)

## Mode

with the human: a conditional in the IR changes the style document's format
([ADR-0007](../../adr/0007-harness.md)).
