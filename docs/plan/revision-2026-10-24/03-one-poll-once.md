# Ticket: Run a future whose ports never wait in one place, the same way everywhere

## What to build

Every caller that runs the frontend or the engine synchronously polls the future with a no-op
waker, and each has written that itself: `block_on` in `crates/cli/src/files.rs:47-56`, `now` in
`crates/bindings-wasm/src/lib.rs:141-150`, `block_on` in `crates/mapfile/src/test_support.rs:25`,
the tests of `crates/engine/tests/render.rs:27` and `crates/adapter-geojson/src/lib.rs:140`, and the
fuzz targets `fuzz/fuzz_targets/mapfile.rs:48-54` and `fuzz/fuzz_targets/render.rs:159-165`.
They differ where it matters: the bindings return `None` for a future that waits, which the
review of #6 asked for because a waiting future spun the tab, while the CLI's `block_on` still
loops on `Poll::Pending` with `yield_now`, so a loader that waits once would hang the command.

One function in `ports`, where async lives
([ADR-0009](../../adr/0009-ports-and-adapters-crates.md)),
polls once and returns `None` for a future that waits; every caller above uses it, and the CLI
reports a waiting port as the bindings do.

## Acceptance criteria

- [ ] Given a future that is ready on its first poll, when the function runs it, then it returns
  the output.
- [ ] Given a future that returns `Poll::Pending` once, when the function runs it, then it
  returns `None` without polling again.
- [ ] Given a resource loader that waits, when the CLI reads a mapfile through it, then it exits
  non-zero with the `port-waited` diagnostic instead of hanging (a test of the CLI's loader seam).
- [ ] Given the tree, when `Waker::noop` is searched for, then it appears only in that function.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `crates/ports/src/lib.rs`;
  `crates/bindings-wasm/src/lib.rs` (`now`, `waited`) is the behaviour to keep.

## Blocked by

<!-- Both change fuzz/fuzz_targets/mapfile.rs. -->

- docs/plan/ir-completeness/02-style-document-numbers-round-trip.md

## Mode

with the human: it adds to the public surface of `ports`, beside the port traits the harness
keeps with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
