# Ticket: Read numbers as C does in one place

## What to build

The frontend, `expr`, and the engine each port C's `atof` and MapServer's `MS_NINT` their own way: a
second, decimal-only `atof` in `crates/mapfile/src/expression.rs` folds `0x10` to 0 where `expr`'s
reads 16; `MS_NINT` saturates in `classes.rs`, keeps the low bits in `labels.rs`, and fails in
`expr`. [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md), proposed, decides one home. Once
the maintainer has settled its open questions, move each copy onto it and drop the re-export of
`atof`, `atoi`, `SIZE_LIMIT`, and `MAX_PATTERN_BYTES` from `expr` that nothing outside it needs.

## Acceptance criteria

- [ ] Given `([n] IN "0x10,1")` and a feature with `n` of 16, when the frontend loads it and the
  engine evaluates it, then the feature is selected, as `expr`'s `atof` reads it.
- [ ] Given a style `OPACITY` bound to `3e9`, when it is drawn, then the engine and the labels round
  it alike, as ADR-0037's answer to its question 3 says.
- [ ] Given the workspace, when the test of ADR-0037's *Enforcement* runs, then no copy is left.

## Read first

- [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0031](../../adr/0031-expression-evaluator.md); `crates/expr/src/c.rs`; `atof` in
  `crates/mapfile/src/expression.rs`; `strtod_reads` in `crates/mapfile/src/wkt.rs`; `ms_nint` and
  `lrint_int` in `crates/engine/src/classes.rs` and `labels.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it implements a proposed ADR whose open questions the maintainer answers first
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 4).
