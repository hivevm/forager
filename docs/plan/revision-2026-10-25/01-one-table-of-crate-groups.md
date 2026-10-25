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

The branch of revision-2026-10-24 ticket 03 (#79) adds `only_poll_once_polls_with_a_no_op_waker`
to the same file. The overlap is textual only, since that test reads no crate list; the session
starts from a `main` that holds #79 or, if #79 is still open, takes it up when it merges.


## Acceptance criteria

- [ ] Given the table of groups in `crates/arch-test/tests/dependencies.rs`, when its own test
  runs, then it asserts that each crate directory under `crates/` appears in the table exactly
  once, with one of ADR-0009's groups, and that the edges, the pure-Rust set, and the set held
  to the Q-2 ban are each computed from the table by a function of it, with no crate named
  outside it.
- [ ] Given the existing tests of `dependencies.rs`, when the change lands, then they pass
  unchanged in what they assert, and the table test
  (`the_rules_reject_a_wrong_edge_and_an_unnamed_crate`) also asserts that a core crate is in
  the pure-Rust set and the Q-2 set.
- [ ] Given a copy of the table with one core crate moved to the hosts, when the table test
  derives the sets from that copy, then the crate is no longer in the pure-Rust set or the Q-2
  set and may depend on an adapter, all without a second edit; the same functions applied to the
  real table give today's sets.

## Read first

- [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0015](../../adr/0015-agg-rust-rasterizer.md), Q-1 and Q-2 of
  `docs/SPECIFICATION.md`.
- `crates/arch-test/tests/dependencies.rs`; the PR body of #77.

## Blocked by

- None (can start immediately)

## Mode

unattended: the table test and the existing tests of `dependencies.rs` decide every criterion.
An interface-preserving refactoring of one test file, isolated and undone by one revert; it
touches nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open
agent branches of [ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap
allows. A wrong outcome shows as a failing existing test, or as a derived set the reviewer finds
to differ from ADR-0009 when reading the table against it; the existing tests passing unchanged
in what they assert is what proves it right. Stop when a derived set would differ from today's
in any crate: that is a change of rule, not a refactoring, and goes to the human.
