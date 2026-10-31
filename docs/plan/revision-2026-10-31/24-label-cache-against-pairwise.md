# Ticket: Hold the label cache's grid to MapServer's pairwise outcome on arbitrary labels

## What to build

[ADR-0035](../../adr/0035-label-cache-grid-and-bound.md) decides that the grid only spares pairs
that cannot meet: its outcome is the pairwise test's. The two blocking findings of #123's review, an
all-`NaN` box missed across cells and boxes copied into every cell, were found by reading, not by a
test; the tests of `crates/label` pick their boxes by hand. A test that draws labels of arbitrary
boxes, `NaN`, infinite, inside-out, and larger than a cell among them, through the cache and through
a pairwise reference of `msTestLabelCacheCollisions` kept in the test, and compares the two
outcomes, would have caught the first; the visit counter of `cache_bounds.rs` over the same inputs,
the second. A fuzz target over the same comparison may follow.

## Acceptance criteria

- [ ] Given the cache as it is, when the comparison runs over a fixed seed's 10,000 inputs, then the
  outcomes agree.
- [ ] Given the grid with the old `NaN` defect put back by hand, when the comparison runs, then it
  fails, as a check that it can see the defect.

## Read first

- [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md); `crates/label/src/cache.rs`;
  `crates/label/tests/cache.rs` and `cache_bounds.rs`; `msTestLabelCacheCollisions` at the pin.

## Blocked by

- None (can start immediately)

## Mode

unattended: the comparison itself, cited `Verifies: ADR-0035` decides its criteria; its scope is a
test of `crates/label`, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a failing comparison; stop: after two attempts that leave
`scripts/check-all.sh` red, or when the comparison finds a disagreement in the cache as it is: that
is a defect, filed, not fixed here, the session leaves its state in this file and hands off.
