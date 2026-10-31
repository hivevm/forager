# Ticket: Read every file the CLI is handed through one bounded reader

## What to build

The CLI has two bounded readers: `read_bounded` in `crates/cli/src/files.rs`, for the base context,
refuses a file that is not regular before it opens it, because opening a pipe or a device may wait
or never end; `read_bounded` in `crates/cli/src/lib.rs`, for `--source` and `--epsg`, opens and
takes, and its comment says that a device or a pipe cannot exhaust memory, while `--source` naming a
FIFO blocks the CLI forever. One reader, the stricter, for both.

## Acceptance criteria

- [ ] Given `--source layer=<fifo>`, when `forager render` runs, then it fails at once with the
  diagnostic of a file that is not regular.
- [ ] Given `--epsg` of a file past its limit, when it runs, then it fails with the limit's
  diagnostic, as today.

## Read first

- `read_bounded` in `crates/cli/src/files.rs` and `crates/cli/src/lib.rs`;
  `docs/plan/revision-2026-10-24/31-base-context-hardening.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a resource limit and the reading of the host's files, which ADR-0007 keeps
with the maintainer.
