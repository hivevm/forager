# ADR-0037: C's reading of numbers is ported once, in `style-ir`, for the frontend and the core alike

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** the ports of C's `atof`, `strtod`, `atoi`, and MapServer's `MS_NINT` in `crates/style-ir`, `crates/expr`, `crates/mapfile`, and `crates/engine`

## Context

MapServer reads numbers with the C runtime: `atof` and `strtod` for a literal, a bound value, and a
WKT coordinate, `atoi` for an integer binding, `MS_NINT` to round one. Forager ports them where
each crate first needed them, and [ADR-0009](0009-ports-and-adapters-crates.md) gives the frontend
no edge to `expr`, so the design revision of 2026-10-31 found them ported several times, apart:

- `atof` in full in `crates/expr/src/c.rs` (hexadecimal, `inf`, `nan`, correctly rounded), and a
  second, decimal only, in `crates/mapfile/src/expression.rs`, which reads a literal and folds a
  numeric `IN` list at load; a third reading of where `strtod` differs from Rust's parser is in
  `crates/mapfile/src/wkt.rs`. `([n] IN "0x10")` reads 0 in the frontend and 16 in `expr`.
- `MS_NINT` three times with three behaviours past the range of a C `long`: saturating in
  `crates/engine/src/classes.rs`, keeping the low bits in `crates/engine/src/labels.rs`, and
  failing as undefined in `crates/expr/src/lib.rs`.
- `expr` re-exports `atof` and `atoi` for the engine's bindings, which evaluate nothing, so the
  evaluator crate has become the C runtime of the engine without a decision saying so.

The convention that a port from MapServer is read in its source and each branch tested is
defeated by copies: a branch fixed in one copy stays wrong in the others. Forces: Q-4 (the same
reading of a number everywhere MapServer reads it the same), ADR-0009's edges, ADR-0031, which
places C's functions that `expr` needs in `expr`.

Questions the maintainer settled on acceptance:

1. `style-ir` is the IR's data model; is it the right home for behaviour every crate may call, or
   should a leaf crate (say `c-runtime`) take it, which supersedes ADR-0009 for one more row and
   edge?
2. Should the frontend fold a numeric `IN` list at all, or keep it as text and let `expr` read
   each item when it evaluates, which would take the frontend's `atof` away without a new home?
3. Which behaviour of `MS_NINT` past a `long` is the pin's on the targets Forager builds for:
   C leaves it undefined, and the three copies chose three answers.

The decision answers the first: the module lives in `style-ir`, with no new crate. The second and
third are left to the ticket that implements it, which reads them from the pin.

## Decision

We will port C's `atof`, `strtod`'s reading of a number's end, `atoi`, and MapServer's `MS_NINT`
once, in a module of `crates/style-ir`, and have the frontend, `expr`, and the engine call that
module instead of copies of their own.

**Out of scope:** `printf` and `strptime`, which only `expr` needs and which stay in `expr`
([ADR-0031](0031-expression-evaluator.md)); the lexer's reading of a number token, which ticket 28
of the revision of 2026-10-24 takes up.

## Alternatives considered

- **A leaf crate `c-runtime`** — the cleanest responsibility, and a supersession of ADR-0009 for a
  module of a few hundred lines.
- **Keep the copies and test them against each other** — no move, and three ports to keep in step
  with each branch the pin takes.
- **Let the frontend depend on `expr`** — the frontend gains the evaluator and `regex` it never
  runs, against ADR-0009 and ADR-0031's "the only crate that uses `regex`".

## Sources / Prior art

- glibc's `strtod` and `atof`, and MapServer's `MS_NINT` at the pin, as the existing ports cite
  them in their comments; where MapServer calls each was not re-read for this proposal and is
  the first step of its implementation.
- The design revision of 2026-10-31, its survey of `crates/mapfile`, `crates/expr`, and
  `crates/engine`.

## Consequences

- Positive: one port per C function, each branch tested once; the frontend and the evaluator read
  `0x10` alike; `expr`'s public surface shrinks to evaluation.
- Negative / trade-offs: the IR crate carries behaviour that is not the data model; every crate
  depends on it already, so no edge is added.
- Follow-ups: the ticket that moves the copies onto the module, and the choice of question 3 as a
  test of its own.

## Enforcement

A test in `crates/arch-test` refuses a function named `atof`, `atoi`, `ms_nint`, or `lrint` defined
outside the module of `style-ir`, citing this ADR with `Verifies: ADR-0037`; the module's tests
cover each branch of the C functions it ports.
