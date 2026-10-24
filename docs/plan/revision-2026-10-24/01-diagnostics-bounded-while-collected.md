# Ticket: Bound the diagnostics of the lexer, the parser, and the translation while they are collected

## What to build

A mapfile load holds at most a thousand diagnostics at any moment, whichever stage reports them.
Today only the reading of files goes through the bounded `Report`
(`crates/mapfile/src/report.rs:1-2` says an input "must not hold them all before the last
thousand are cut"). The lexer (`crates/mapfile/src/lexer.rs:57`), the parser
(`crates/mapfile/src/syntax.rs:203-240`), and the translation
(`crates/mapfile/src/translate.rs:25-33`) each collect into a `Vec<Diagnostic>` of their own,
which `load` hands to the report only afterwards (`crates/mapfile/src/lib.rs:49-54`).

Measured on `main` 9d2a9bc with `forager read` (release build): 16 MB of `UNITS DD` lines inside
`MAP` yield 1,777,778 `unsupported-keyword` warnings, of which 1,000 are printed, and peak at
1.0 GB resident; the same size of input with a single diagnostic (`FROB 1` lines) peaks at
645 MB. The difference, about 350 MB, is diagnostics that are cut once the load has finished.
Each stage pushes into the one `Report` instead, so the cut happens while they are collected.

## Acceptance criteria

- [ ] Given a mapfile of 16 MB of `UNITS DD` lines inside `MAP`, when it is loaded through
  `load`, then the peak of heap memory a counting allocator measures over the load is no more
  than 50 MB above that of an input of the same size with no diagnostic, and the load returns
  `MAX_DIAGNOSTICS` diagnostics and a last `diagnostics-truncated`.
- [ ] Given more than `MAX_DIAGNOSTICS` warnings from the translation, when the load ends, then
  `diagnostics-truncated` counts every one that was dropped, as it does for those from reading.
- [ ] Given the existing tests of `crates/mapfile`, when they run, then they pass unchanged.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md) and the Q-9 criterion of
  `docs/SPECIFICATION.md`.
- `crates/mapfile/src/report.rs`, `crates/mapfile/src/lib.rs`; the test
  `diagnostics_are_bounded_while_they_are_collected` in `crates/mapfile/src/tests.rs` is the
  pattern; the counting allocator goes in an integration test of its own (`tests/`), since a
  global allocator is one per test binary.

## Blocked by

- None (can start immediately)

## Mode

with the human: the resource limits are never worked unattended
([ADR-0007](../../adr/0007-harness.md)).
