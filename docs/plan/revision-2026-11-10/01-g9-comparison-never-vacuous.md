# Ticket: Hold the G-9 comparison to a case it actually compares, with its regression tests

## What to build

The test of G-9, `every_case_in_scope_renders_alike_natively_and_on_wasm32` in
`crates/cli/tests/msautotest.rs`, first wrote nothing into a layer whose shapefile did not open
or read, so both renders drew the same empty layer and agreed; 22d7aec made `inline` of
`crates/cli/tests/conformance/wasm.rs` return an error naming each such shapefile, and the test
counts it as a difference. The fix shipped without the regression test
[`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done) asks for: nothing that runs
without the suite fails when `inline` goes back to skipping such a layer.

The same hole is still open one step later: `difference` holds two renders that give no image for
the same reason as alike (`(Err(a), Err(b)) if a == b => None`), and the test counts such a case
as compared. A case in scope has a reference image, so a render that gives none, natively and in
the module alike, is a comparison of nothing, as the empty layers were.

Add the regression test for `inline`, and let the test of G-9 count a case in scope that renders
no image natively as a difference, whatever the module gives; `difference` itself may keep its
answer for two equal errors if the test decides it before calling it, or change it, whichever is
simpler.

## Acceptance criteria

- [ ] Given a map whose local layer, drawn by the engine, names a shapefile the loader does not
  hold, when `inline` runs, then it returns an error naming that path; the test runs without the
  suite, beside the unit test of `difference` in `crates/cli/tests/conformance/tests.rs`, and
  fails against `inline` as it was before 22d7aec.
- [ ] Given a case in scope whose native render gives no image, when the test of G-9 runs, then
  the case is reported among the differences with the native render's reason, and a unit test
  without the suite decides the rule that reports it.
- [ ] Given the suite and the module, when `scripts/check-rust.sh` runs, then all cases in scope
  still render alike and the count of compared cases equals the count in scope.

## Read first

- [ADR-0043](../../adr/0043-wasm-renders-compared-in-wasmi.md),
  [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md);
  `crates/cli/tests/conformance/wasm.rs`, `crates/cli/tests/conformance/tests.rs`, the test of G-9
  in `crates/cli/tests/msautotest.rs`; the convention in `docs/CONVENTIONS.md` that a comparison
  fails when there is nothing to compare.

## Blocked by

- None (can start immediately)

## Mode

unattended: unit tests without the suite decide both rules, and the run of
`scripts/check-rust.sh` with the suite shows the comparison still green; the scope is the
harness's test code alone, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of
three; stop: when a case in scope turns out to render no image natively, which would be a defect
of the harness or the list rather than of this ticket, the session records the case here and
hands off.
