# Ticket: Name the crate groups of ADR-0009 once in the dependency test

## What to build

`crates/arch-test/tests/dependencies.rs` writes the crates of the IR and the core three times: the
`CORE` list that `engine` may depend on, the `must_be_pure_rust` match, and the `IR_AND_CORE` list
of the Q-2 ban, which pull request #77 added and flagged as its doubt 4. A crate added to one and
forgotten in another escapes either the edge rule, the C-code rule, or the ban without a failing
test. One table of ADR-0009's groups, each crate with its group, from which the three sets are
derived, makes the groups a single fact. The hosts list (`HOSTS`) belongs in the same table.
`scripts/check-rust.sh` keeps a fourth list, `NATIVE_ONLY`, of the crates never built for the
browser; it is the hosts with a file system and the tooling, and stays in the script, which
cannot read the Rust table, unless the session finds a simple way to share it.

## Acceptance criteria

- [ ] Given the dependency test, when it is read, then each crate ADR-0009 names appears in it
  once, with its group, and the edges, the pure-Rust set, and the set held to the Q-2 ban are
  derived from the groups.
- [ ] Given the existing tests of `dependencies.rs`, when the change lands, then they pass
  unchanged in what they assert, and the table test
  (`the_rules_reject_a_wrong_edge_and_an_unnamed_crate`) also asserts that a core crate is in
  the pure-Rust set and the Q-2 set.
- [ ] Given a crate moved from one group to another in the table, when the tests run, then every
  rule follows the move without a second edit.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0015](../../adr/0015-agg-rust-rasterizer.md), Q-1 and Q-2 of
  `docs/SPECIFICATION.md`.
- `crates/arch-test/tests/dependencies.rs`; the PR body of #77.

## Blocked by

- None (can start immediately)

## Mode

unattended: an interface-preserving refactoring of one test file. The existing tests decide it;
a wrong result costs one revert and is noticed by those tests or by the reviewer reading the
table against ADR-0009. Stop when a derived set would differ from today's in any crate: that is
a change of rule, not a refactoring, and goes to the human.
