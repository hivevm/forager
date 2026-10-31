# Ticket: Find why a worktree's build artifacts break `check-all.sh`, and name the cure

## What to build

During the spec `labels-2`, `scripts/check-all.sh` failed in a ticket's worktree on build
artifacts, not on the change: once on a test binary filled with zeros ("Exec format error"), and
twice in #155 on the `wasm32-unknown-unknown` artifacts ("can't find crate"), which passed once that
worktree's `target/wasm32-unknown-unknown` was removed. Each session lost time on it, and a
session that did not know the cure could have taken the failure for its own, or pointed
`CARGO_TARGET_DIR` at another tree's artifacts. Find the cause: an interrupted link, a disk full,
a toolchain or target updated under the worktree, several worktrees' builds at once. Then either
remove it, or let `scripts/check-rust.sh` recognize the two failures and name the directory to
remove.

## Acceptance criteria

- [ ] Given the cause found, when it is written here, then it names how it is reproduced or why
  it cannot be.
- [ ] Given a worktree whose artifacts carry either fault, when `check-all.sh` runs, then it
  either does not fail on it, or fails naming the directory to remove.

## Read first

- [ADR-0008](../../adr/0008-rust-toolchain.md), [ADR-0010](../../adr/0010-rust-dev-container.md),
  [ADR-0007](../../adr/0007-harness.md) (the worktrees); `scripts/check-rust.sh`,
  `scripts/build-web.sh`; #155's last doubt (`gh pr view 155`).

## Blocked by

- None (can start immediately)

## Mode

with the human: the cause is not known, and it may lie in the Dev Container or the harness, which
the maintainer decides.
