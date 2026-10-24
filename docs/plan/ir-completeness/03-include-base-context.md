# Ticket: Let the CLI take a base context above the mapfile's directory

## What to build

`forager read --base <dir> <mapfile>` and `forager render --base <dir> …` resolve `INCLUDE`,
and every other resource path, inside `<dir>` instead of the mapfile's directory, so that
`INCLUDE "../shared.inc"` loads when `..` stays inside the base. The mapfile itself must lie
inside the base; without `--base`, the base stays the mapfile's directory. Q-8's diagnostics for
a path that leaves the base, absolute paths, and symbolic links out of it stay as they are.

## Acceptance criteria

- [ ] Given a mapfile in `base/maps/` with `INCLUDE "../inc/a.inc"` and `--base base`, when
  `forager read` runs, then the include is spliced in and the load succeeds.
- [ ] Given the same mapfile without `--base`, when `forager read` runs, then the include is
  refused as today (citing Q-8).
- [ ] Given `--base base` and `INCLUDE "../../outside.inc"`, when `forager read` runs, then it is
  refused with the existing diagnostic.
- [ ] Given a mapfile outside `--base`, when the CLI starts, then it exits non-zero naming both.

## Read first

- Q-8 of `docs/SPECIFICATION.md`; the *Base context* and *Resource loader* entries of
  `docs/GLOSSARY.md`; [ADR-0009](../../adr/0009-ports-and-adapters-crates.md).
- The CLI's file loader and its tests; the frontend test
  `an_include_outside_the_base_context_is_refused`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the resolution of resource paths is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
