# Ticket: Find why `attrbind_test001`'s "Label 1" stands one pixel off MapServer's

## What to build

`misc/attrbind.map attrbind_test001.png` passes, but not `labels-hold`: its label "Label 1" stands
1.00 px above MapServer's record (`crates/cli/tests/conformance/cases.txt`), so G-10's "within
1 px" holds there only at the edge. #155 placed the cause in the point placement, not in the
labelpoly, and did not trace it. Find the step of `msDrawLabelCache` or `pointLayerDrawShape` at
the pin where the engine's arithmetic leaves MapServer's for this label, and fix it there.

## Acceptance criteria

- [ ] Given `attrbind_test001`, when the harness runs, then the label stands where MapServer's
  record has it, and the case is marked `labels-hold`.
- [ ] Given the step found, when its test runs, then it holds the value worked out from the C,
  and fails against today's code.

## Read first

- G-10 in `docs/SPECIFICATION.md`;
  [ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md);
  `crates/cli/tests/conformance/labels/` (the record); `crates/label/src/place.rs`;
  `docs/CONVENTIONS.md` (behaviour ported from MapServer, a test holds a value it states).

## Blocked by

- None (can start immediately)

## Mode

unattended: the harness's mark and an engine test decide it; the fix is one step of the
placement in `crates/label` or the engine, undone by one revert. It touches nothing
[ADR-0007](../../adr/0007-harness.md) keeps with the human: no resource limit, port, public
interface, or style document format. Its branch counts against ADR-0007's cap of three open agent
branches. A wrong result moves another case's outcome or mark, and is undone by the revert; the
case marked `labels-hold` and the test of the step prove it right. Stop: if two hours of
tracing find no step, or the fix moves any other case's outcome or mark, record what was ruled
out here and hand off.
