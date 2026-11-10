# Ticket: Let a label box with an edge that is no number never match in the label comparisons

## What to build

`distance` in `crates/cli/tests/conformance/labels.rs` folds the differences of a box's edges
with `f64::max`, which returns the other operand when one is NaN. An edge that is no number, in
Forager's box or in a record, gives a NaN difference that the fold drops, so the distance is that
of the other edges, 0 when they agree, and the pair matches within `TOLERANCE`. `compare`, which
decides G-10 and the mark `labels-hold`, and `beyond_noise`, which holds the reader's output to
its record within `NOISE`, both go through it
([ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md)).
A record is parsed with `str::parse::<f64>`, which reads `nan` and `inf`, so a `%.17g` of a NaN
in MapServer's output reads back as one.

The label cache made the same mistake, filing a box with a NaN edge at its point because
`f64::min` dropped the NaN, until f8fa86d; the label cache meets such boxes (an `ANGLE` bound to
`nan`, the case f8fa86d tests), and whether one reaches `Rendered.labels` drawn is not what the
comparison should depend on. A comparison that a NaN passes holds nothing about the label it
names. Let a pair whose boxes are not finite in every edge differ, in both comparisons, with a
message that says which edge is no number.

## Acceptance criteria

- [ ] Given a Forager label drawn at a box with one NaN edge and MapServer's label of the same key
  at a box whose other edges agree, when `compare` runs, then it reports the pair as a difference;
  a unit test beside the tests of `compare` in `crates/cli/tests/conformance/tests.rs`, run
  without the suite, states it, and fails against `distance` as it is.
- [ ] Given a record and a reader's output whose boxes agree but for one NaN or infinite edge in
  either, when `beyond_noise` runs, then it reports the line, decided by a unit test.
- [ ] Given the suite, when the harness runs, then the marks `labels-hold` of `cases.txt` do not
  move.

## Read first

- [ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md);
  `crates/cli/tests/conformance/labels.rs` (`distance`, `compare`, `beyond_noise`, `read`), the
  unit tests in `crates/cli/tests/conformance/tests.rs`; the conventions in `docs/CONVENTIONS.md`
  that a comparison fails when there is nothing to compare and on minimum and maximum over values
  that may be NaN.

## Blocked by

- None (can start immediately)

## Mode

unattended: unit tests without the suite decide both comparisons, and the harness with the suite
shows no mark moving; the scope is the harness's test code alone, and a wrong result costs one
revert. It touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the
maintainer and counts against its cap of three. Stop: when a mark `labels-hold` moves, which would
mean a case held today by a NaN, the session records the case here and hands off.
