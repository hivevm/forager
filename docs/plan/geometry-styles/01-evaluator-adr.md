# Ticket: Decide the expression evaluator and its regular expressions

## What to build

An ADR, through `propose-adr`, for the core crate `expr` of ADR-0009: how it evaluates the IR's
CQL2 tree against a feature's properties as MapServer's `msEvalExpression` and `mapparser.y` do at
the pin (types, `atof`, string comparison, `casei`, `in`, arithmetic, time, the string
functions), and how `regexp_like` matches POSIX extended regular expressions as MapServer's
`regcomp`/`regexec` do: a crate, weighed for `wasm32-unknown-unknown`, licence, maintenance, and
its divergence from POSIX, or an implementation of its own.

## Acceptance criteria

- [ ] Given the proposed ADR, then it names how each operator and function of ADR-0030 evaluates,
      from MapServer's source, and the regular-expression engine with its alternatives.
- [ ] Given the ADR, then its Enforcement names the tests that hold it.

## Read first

- ADR-0009, ADR-0012, ADR-0030, ADR-0026; `docs/semantics/expression.md`.
- MapServer at the pin: `msEvalExpression` (`src/maputil.c`), `src/mapparser.y`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides an ADR, the maintainer's (ADR-0007).
