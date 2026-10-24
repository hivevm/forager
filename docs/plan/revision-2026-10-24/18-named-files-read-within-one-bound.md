# Ticket: Read every file a mapfile names through one bounded read

## What to build

`Reading` in `crates/mapfile/src/lib.rs` loads a file, adds its length to the running total, and
refuses it past `MAX_TOTAL_BYTES` in three places written apart: `read` for the mapfile, its
`INCLUDE`s, and the symbol file (`lib.rs:238-291`), `font_set` (`lib.rs:107-131`), and
`init_files` (`lib.rs:136-183`). Each words the refusal on its own: a symbol file past the bound is
reported as "the mapfile with its INCLUDEs exceeds", a font set or init file as "the mapfile with
its files exceeds". A file kind added later copies one of the three, and a bound fixed in one is
missed in the others.

One method of `Reading` loads a resource path within the bound and reports the refusal in one
wording, with the span of what named the file; `read`, `font_set`, and `init_files` use it.
Diagnostic codes do not change.

## Acceptance criteria

- [ ] Given the existing tests of `crates/mapfile`, when they run, then they pass, except a test
  that matches the old wording of `mapfile-too-large`, which matches on the code instead.
- [ ] Given a symbol file, a font set, and an init file each pushing the total past
  `MAX_TOTAL_BYTES`, when each is loaded, then each yields `mapfile-too-large` with the span of
  the directive or projection that names it.
- [ ] Given `lib.rs`, when `self.bytes > MAX_TOTAL_BYTES` is searched for, then it appears once.

## Read first

- [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md); Q-8 and Q-9 of
  `docs/SPECIFICATION.md`.
- `crates/mapfile/src/lib.rs`, the bound tests of `crates/mapfile/src/tests.rs`.

## Blocked by

- docs/plan/revision-2026-10-24/01-diagnostics-bounded-while-collected.md

## Mode

with the human: the size bound is a resource limit of Q-9, kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
