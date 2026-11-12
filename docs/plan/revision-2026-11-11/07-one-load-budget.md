# Ticket: Hold a load's bounds of Q-9 by one budget type, the defaults' bound with its own name

## What to build

A load holds two bounds of Q-9 on text, each 16 MiB, in two counters written apart. The text the
loader reads is counted by `Loader.bytes`, `over_limit()`, and the report-once of `load_bounded`
(`crates/mapfile/src/lib.rs:175-204`). What the defaults of `VALIDATION` add is counted by
`Translator.default_bytes` (`crates/mapfile/src/translate/mod.rs:198`), its guard against charging
on at `mod.rs:800-804`, its check and error at `translate/defaults.rs:68-79`. Only the second
guards against a sum that wraps on `wasm32`. Both compare against `MAX_TOTAL_BYTES`, documented as
the bound on a mapfile with its `INCLUDE`s. The defaults' error reuses `mapfile-too-large` and says
the defaults add text "to the mapfile". Let one small budget type in `mapfile` charge, stop at its
bound, and report once, for both. Give the defaults' bound a constant of its own, and decide with
the maintainer whether its diagnostic keeps the code `mapfile-too-large` (PR #177's doubt).

## Acceptance criteria

- [ ] Given a load whose defaults pass their bound in two expressions, when it loads, then it
  reports exactly one diagnostic of the bound.
- [ ] Given a mapfile and its `INCLUDE`s past 16 MiB, when it loads, then it reports exactly one
  `mapfile-too-large`, as at the base.
- [ ] Given `crates/mapfile/src`, when it is searched, then no comparison against a bound of Q-9's
  text stands outside the budget type.

## Read first

- Touches: `crates/mapfile/src/lib.rs`, `crates/mapfile/src/translate/mod.rs`,
  `crates/mapfile/src/translate/defaults.rs`
- Q-9 in `docs/SPECIFICATION.md`; `docs/CONVENTIONS.md` (*A count held to a bound of Q-9 stops at
  the bound*, *Diagnostic codes are kebab-case and stable*); `docs/semantics/expression.md` (the
  paragraph on defaults).
- Overlap: `docs/plan/revision-2026-11-11/08-validation-diagnostics.md` edits the same two files
  of `translate/`; whichever lands second takes the other's shape.

## Blocked by

- None (can start immediately)

## Mode

with the human: it touches the resource limits, which
[ADR-0006](../../adr/0006-harness.md) never leaves unattended, and a diagnostic code may change.
