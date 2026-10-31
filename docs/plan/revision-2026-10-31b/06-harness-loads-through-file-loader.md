# Ticket: Load each harness case through the CLI's base-context loader

## What to build

The conformance harness binds a case's sources through the CLI's library since #141, but it still
loads the mapfile through a loader of its own: `Suite` in `crates/cli/tests/msautotest.rs` reads
with a bare `std::fs::read` of the suite's directory joined with the path, without the containment
or the size bound of `FileLoader`, and `ready` repeats `forager_cli::now` with a panic where `now`
gives the diagnostic `port-waited`. So one case is read through two loaders, and its mapfile is not
loaded the way `forager render` loads it, only bound that way. Load it through the
`FileLoader` rooted at the suite, the same one the binding then receives, keeping the case's `-i`
substitution (`MainReplaced`) as a wrapper around it.

## Acceptance criteria

- [ ] Given the conformance harness, when `scripts/check-rust.sh` runs it, then the counts of cases
  in scope, passing, and `labels-hold` are those of today.
- [ ] Given G-1's load of every mapfile, when it runs, then it loads through the same loader and
  every mapfile loads as today.
- [ ] Given `crates/cli/tests/msautotest.rs`, when it is read, then no `ResourceLoader` of its own
  reads the file system.

## Read first

- `crates/cli/tests/msautotest.rs` (`Suite`, `MainReplaced`, `ready`, the conformance render),
  `crates/cli/src/files.rs`, `crates/cli/src/lib.rs`; the conformance paragraph of
  `docs/ARCHITECTURE.md`; [ADR-0027](../../adr/0027-mapserver-built-from-the-pin.md) for the noise
  measurement, which shares the module.

## Blocked by

- None (can start immediately)

## Mode

unattended: the harness's counts decide its criteria; its scope is the harness's loading, and a
wrong result costs one revert; it uses `FileLoader` and changes it in nothing, so it touches none of
the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer, and counts against its
cap of three; a wrong result shows as a moved count; stop: when a case loads differently through
`FileLoader` (a symbolic link, a path out of the suite), the session names it in this file and hands
off.
