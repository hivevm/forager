# Ticket: Harden the base context against what its review left

## What to build

Found by area 6 of the port review (revision ticket 23, #69) and its security review:

- **TOCTOU, low:** the CLI's loader canonicalizes a path, checks it against the base context,
  then opens it by path; one who can write into the base context during a load can swap a
  directory for a symlink, or a file for a FIFO, in between, and the FIFO blocks `open(2)`
  before the check on the opened file. Opening with `O_NOFOLLOW` walking or by `openat`, and
  with `O_NONBLOCK` on unix, closes it; `O_NONBLOCK` needs `libc`, a new dependency
  ([ADR-0012](../../adr/0012-crate-supply-chain.md)).
- **URL loaders, medium if one is built:** `%2e%2e` is a legitimate file name for
  `ResourcePath`, and a URL parser reads it as `..`; a loader that builds URLs percent-encodes
  each segment. The rule belongs on `ResourcePath` or the port's documentation.
- **Windows, untested:** reserved names (`CON`, `NUL`), trailing dots, alternate data streams,
  and case-insensitive file systems rest only on the canonical-path check; a test runs on the
  Windows CI job.
- **Budget per file, info:** each file may be read up to `MAX_TOTAL_BYTES + 1`, not what remains
  of the bound, so the peak is about twice the bound.

## Acceptance criteria

- [ ] Given a symlink or a FIFO swapped in during a load, when the file is opened, then it is
  refused, a test each where the platform allows it.
- [ ] Given a URL loader's contract, when it is read, then it requires each segment
  percent-encoded.
- [ ] Given the Windows CI job, when it runs, then the reserved names, trailing dots, and streams
  are refused, a test each.

## Read first

- Q-8 and Q-9 of `docs/SPECIFICATION.md`; `crates/ports/src/lib.rs`, `crates/cli/src/files.rs`,
  `crates/cli/tests/base_context.rs`; the PR #69 text.

## Blocked by

- None (can start immediately)

## Mode

with the human: resource-path resolution (Q-8) and a new dependency are the maintainer's
([ADR-0007](../../adr/0007-harness.md)).
