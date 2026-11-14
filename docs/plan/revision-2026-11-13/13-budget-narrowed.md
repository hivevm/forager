# Ticket: Let the load's budget decide what it reports

## What to build

`crates/mapfile/src/budget.rs` (50b933f) charges a count and answers `Within`, `PassedNow`, or
`Passed`. Both callers ask `passed()` before they charge (`crates/mapfile/src/lib.rs:194`,
`crates/mapfile/src/translate/mod.rs:956`), so `Passed` never comes back: the arm at `lib.rs:211`
cannot run, and `translate/defaults.rs:82` folds it into `!= Within`. `admits()` and `bound()`
serve only the defaults' check of one expression, a bound of its own on the same number. Let
`charge` own "report once", or fold the type into a saturating count; a file past the bound stays
refused unread, as `load_bounded`'s doc says, so the pre-check of `lib.rs:194` stays where it saves
the read;
keep the one expression's check a comparison in `defaults.rs`. No behaviour changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the budget is narrowed, then it passes unchanged, the Q-9
  tests of the load's text among them.
- [ ] Given the workspace, when the change lands, then no match arm of a charge's outcome is
  unreachable.

## Read first

- Touches: `crates/mapfile/src/budget.rs`, `crates/mapfile/src/lib.rs`,
  `crates/mapfile/src/translate/mod.rs`, `crates/mapfile/src/translate/defaults.rs`
- `docs/CONVENTIONS.md`, *A count held to a bound of Q-9 stops at the bound* and *A bound of Q-9
  reports once*.

## Blocked by

- docs/plan/revision-2026-11-13/10-hash-table-reader-once.md

## Mode

with the human: the resource limits are kept with the maintainer by ADR-0006, even where no value
moves.
