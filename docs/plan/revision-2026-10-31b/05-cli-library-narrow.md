# Ticket: Narrow the CLI's library to what its two users call

## What to build

Since #141, `forager-cli` has a library (`crates/cli/src/lib.rs`) that the binary and the
conformance harness render through. Its surface is wider than they need:

- `FileLoader::for_data` is `pub` (`crates/cli/src/files.rs`) and returns `DataLoader`, which is
  `pub` inside the private module `files` and not re-exported: callable outside the crate, its type
  unnameable there. Only `lib.rs` calls it.
- `render` reports diagnostics on two channels, its `&mut Vec<Diagnostic>` parameter for the
  binding and `Rendered.diagnostics` for the engine, and both callers merge them by hand
  (`main.rs`, `crates/cli/tests/msautotest.rs`).

Make `for_data` and `DataLoader` crate-private and return the binding's diagnostics with the
rendering, in the order the binary prints them today. `now` and `read_bounded`, public only for the
binary, stay to the open ticket `docs/plan/revision-2026-10-31/21-cli-one-bounded-reader.md`.

## Acceptance criteria

- [ ] Given `forager render` over the fixtures of `crates/cli/tests/render.rs`, when it runs, then
  its standard error and its image are byte for byte those of today.
- [ ] Given the conformance harness, when it runs, then its counts and its label comparison are
  unchanged.
- [ ] Given `cargo doc -p forager-cli`, when it is read, then `for_data` and `DataLoader` are not
  in it and `render` takes no output parameter.

## Read first

- `crates/cli/src/lib.rs`, `files.rs`, `main.rs`; the conformance render in
  `crates/cli/tests/msautotest.rs`; the `cli` entry of `docs/ARCHITECTURE.md`; the entry on what a
  ticket's pull request carries in `docs/CONVENTIONS.md` (#141's review dropped a trait added to
  `files.rs` beyond its ticket).

## Blocked by

- None (can start immediately)

## Mode

unattended: the CLI's tests and the harness decide its criteria; its scope is the CLI crate's
library surface and its two callers, and a wrong result costs one revert; it changes neither the
resolution of resource paths nor a limit, which [ADR-0007](../../adr/0007-harness.md) keeps with
the maintainer, and counts against its cap of three; a wrong result shows as a changed diagnostic
or count; stop: when narrowing needs `files.rs` to change beyond the visibility of `for_data`, the
session leaves its state in this file and hands off.
