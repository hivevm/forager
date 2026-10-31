# Ticket: Hold every label record to a text case of the list

## What to build

The G-10 comparison reads MapServer's labels of each text case from a record under
`crates/cli/tests/conformance/labels/`, at the path `record_path` gives for the case. Nothing fails
on a record that no text case of `cases.txt` reads: a case that leaves the list, goes out of scope,
or changes its category to geometry leaves its record behind, and the next reader takes it for
measured (#137, its last doubt). Today the 16 records and the 16 text cases line up. A test that
runs on every push, without the suite, holds the two sets equal from `cases.txt` and the directory
alone, and names each record without a case and each text case without a record.

## Acceptance criteria

- [ ] Given a record whose case is not a text case in scope in `cases.txt`, when the test runs,
  then it fails naming the record.
- [ ] Given a text case in scope with no record, when the test runs, then it fails naming the case.
- [ ] Given the tree as it is, when `scripts/check-rust.sh` runs, then the test passes and runs
  without the msautotest checkout.

## Read first

- `crates/cli/tests/conformance/mod.rs`, `labels.rs`, `cases.txt`, and `record_path` in
  `crates/cli/tests/msautotest.rs`; the conformance paragraph of `docs/ARCHITECTURE.md`;
  [ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md); the entry on refusals
  asserting their reason in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the test decides its criteria; its scope is one test of the harness, and a wrong result
costs one revert; it touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer and counts against its cap of three; a wrong result shows as a red push check on a tree
whose records line up; stop: when the list cannot say from its text alone which cases read a
record, the session leaves its state in this file and hands off.
