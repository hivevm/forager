# Ticket: Refuse stray `println!`, `eprintln!`, and `dbg!` in the workspace, its tests included

## What to build

7face19 landed a measuring `eprintln!("MEASURE …")` in `crates/engine/tests/label_memory.rs`; the
next ticket's commit, c0f2906, removed it.
[`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done) asks for no stray debug
output, and no check holds it: the workspace's lints in `Cargo.toml` do not warn
`clippy::print_stdout`, `clippy::print_stderr`, or `clippy::dbg_macro`.

Warn the three lints for the workspace, tests included, so that `scripts/check-rust.sh`, which
denies warnings, fails on a new one; and let each deliberate output say so where it stands, with
`#[expect(…, reason = "…")]`, as `crates/engine/tests/bless/mod.rs` already does for its report:
the CLI's own output, the conformance harness's reports of G-10 and the noise measurement
(`crates/cli/tests/msautotest.rs`), and the dev server's.

## Acceptance criteria

- [ ] Given a `dbg!` or an `eprintln!` added to a test of any crate, when
  `scripts/check-rust.sh` runs, then it fails and names the line.
- [ ] Given today's deliberate outputs, when the checks run, then they pass, each with its reason.
- [ ] Given the fuzz workspace, when `scripts/check-rust.sh` lints it, then the same lints hold
  there, or the pull request says why not.

## Read first

- [ADR-0008](../../adr/0008-rust-toolchain.md) (the lints of the workspace),
  [ADR-0002](../../adr/0002-decisions-verified-by-tests.md); `Cargo.toml`
  (`[workspace.lints]`), `scripts/check-rust.sh`, `crates/engine/tests/bless/mod.rs`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the check decides itself, shown red on an added `dbg!` in the pull request's
description; its scope is the workspace's lint table and the `expect`s, and a wrong result costs
one revert; it touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer and counts against its cap of three; stop: when a crate's output cannot be held by an
`expect` without a wider allow, the session records it here and hands off.
