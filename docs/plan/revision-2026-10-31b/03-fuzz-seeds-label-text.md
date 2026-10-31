# Ticket: Seed the mapfile fuzz target with a label's `TEXT` and characters past ASCII

## What to build

The mapfile fuzz target (`fuzz/fuzz_targets/mapfile.rs`) reaches the reading of a label's or a
class's `TEXT` (`translate/label.rs`, `expression::text`, `text_bindings`), but none of its seeds
under `fuzz/seeds/mapfile/` has one: `inline.map` carries only a `FEATURE`'s `TEXT`, which the
frontend reads as a string, not through the expression scanner. Nor does a seed hold a character
past ASCII. So the panic of `text_bindings` on a multi-byte character after an `f` (fixed in #140)
was a mutation away from no seed, and the weekly run never ran on it: the code came on 2026-10-29
and left on 2026-10-31, between two Sundays. Add seeds whose labels and classes have a `TEXT` with
bindings (`[name]`, `[name]]`-doubled brackets), quoted strings, and multi-byte characters, and a
`CLASS` `EXPRESSION` with each kind of literal, so the target starts where the panic stood.

## Acceptance criteria

- [ ] Given the seeds, when `fuzz/fuzz_targets/mapfile.rs` runs over them, then at least one
  reaches `text_bindings` with a multi-byte character after an `f`, and each loads with its
  diagnostics and no panic.
- [ ] Given the commit of #140's parent with the new seeds, when the target runs over them, then
  it panics in `text_bindings`; this is shown once by hand and said in the pull request.
- [ ] Given `scripts/fuzz.sh`, when it reads the targets, then every target still has seeds.

## Read first

- `fuzz/fuzz_targets/mapfile.rs`, `fuzz/seeds/mapfile/`, `scripts/fuzz.sh` and its self-test
  `scripts/test-fuzz.sh`; Q-3 in `docs/SPECIFICATION.md`; `crates/mapfile/src/expression.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the target run over its seeds and `scripts/check-all.sh` decide its criteria; its scope
is the seed directory, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; a wrong result shows as a seed that panics or loads nothing; stop: after two attempts that
cannot make a seed reach `text_bindings`, the session leaves its state in this file and hands off.
