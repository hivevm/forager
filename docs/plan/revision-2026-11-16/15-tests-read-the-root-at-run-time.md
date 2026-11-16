# Ticket: Tests that read the tree find it at run time, not where they were compiled

## What to build

`scripts/check-rust.sh` failed on this revision's branch in
`crates/arch-test/tests/c_numbers.rs`: "crates/style-ir/src/c.rs defines no `atof`". The cause
was a stale binary in the shared `target/`. Cargo had built it on 2026-11-15 in the linked
worktree `.scratch/worktrees/reprojection-rest`, which no longer exists. The binary held that
worktree's path through `env!("CARGO_MANIFEST_DIR")`, and Cargo judged it fresh for
`cargo test --workspace`. So the test walked a directory that is gone, found no file, and failed
on its own sanity assert. `cargo clean -p forager-arch-test` cleared it.

A test that walks the tree passes or fails on files it did not read. Its own assert caught this
case. A test without that assert would have passed on nothing. A test that reads the tree takes
the root from `std::env::var("CARGO_MANIFEST_DIR")` at run time, which `cargo test` sets for the
package it runs. It falls back to `env!` only where the variable is unset. The tests of
`crates/arch-test` come first. The other users of `env!("CARGO_MANIFEST_DIR")` under
`crates/*/tests` follow where they read files of the tree.

## Acceptance criteria

- [ ] Given a test binary of `arch-test` built in one checkout, when it is run from another
  checkout of the same commit with `CARGO_MANIFEST_DIR` set to that checkout's crate, then it
  reads the second checkout's files.
- [ ] Given the tree, when `grep -rn 'env!("CARGO_MANIFEST_DIR")' crates/arch-test/tests` runs,
  then each hit is a fallback behind the run time's variable.
- [ ] Given `cargo test --workspace`, when it runs, then every test passes unchanged in what it
  asserts.

## Read first

- Touches: `crates/arch-test/tests/*.rs`, `crates/arch-test/tests/module_direction/mod.rs`, and
  the tests under `crates/*/tests` that read the tree through `env!("CARGO_MANIFEST_DIR")`
- ADR-0007 (the checks), ADR-0008 (what `arch-test` holds); `docs/CONVENTIONS.md` (a comparison
  fails when there is nothing to compare).

## Blocked by

- None (can start immediately)

## Mode

unattended: the tests decide it, it changes only how tests find the tree, and a wrong result costs
one revert. Stop if a test's assertion has to change.
