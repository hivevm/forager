# Ticket: Refuse a citation of a superseded or rejected ADR outside the record

## What to build

a9f76f5 superseded ADR-0021 by ADR-0045, and the README went on citing ADR-0021 for the vendored
files of the browser example until the review of the branch found it (22d7aec). Check 5 of
`scripts/check-docs.sh` asks only that a cited ADR exists, and a superseded one does. Two such
citations are still in the tree:

- `.gitattributes`, the comment over `examples/web/vendor/** -text`, cites ADR-0021, superseded by
  [ADR-0045](../../adr/0045-vendored-third-party-files-of-the-example.md);
- `crates/adapter-epsg/Cargo.toml`, the crate's `description`, cites ADR-0018, superseded by
  [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md), and published with the crate.

Inside `docs/adr/` a superseded ADR is cited by right, as the history a successor answers; outside
it, today, nothing cites one but these two, each as the decision in force. A superseded ADR
"explains why; it is not a rule" ([`AGENTS.md` §2](../../../AGENTS.md#2-start-here)), so a reader
sent to it is sent to a decision the project has left.

Add the check: an `ADR-NNNN` reference to an ADR whose status is superseded or rejected, in a
tracked file outside `docs/adr/`, is an error naming the file, the line, and the ADR that
supersedes it where there is one. Correct the two citations in the same change. Should a line
outside the record need to name a superseded ADR as history, the change says how it is told
apart, for example by naming its successor on the same line. `scripts/check-docs.sh` is the
template's, so the check likely belongs there first, as the fix of
docs/plan/revision-2026-10-24/14-revision-sensor-same-day.md does.

## Acceptance criteria

- [ ] Given a fixture with a superseded ADR cited in a README, when `scripts/check-docs.sh` runs,
  then it fails and names the file, the line, and the superseding ADR;
  `scripts/test-check-docs.sh` holds the case.
- [ ] Given the same citation inside another ADR, when the check runs, then it passes.
- [ ] Given this repository after the change, when the check runs, then it passes, with
  `.gitattributes` citing ADR-0045 and the description of `adapter-epsg` citing ADR-0023.

## Read first

- [ADR-0001](../../adr/0001-agent-governance-model.md),
  [ADR-0005](../../adr/0005-template-releases.md), the mechanics of supersession in
  [`docs/adr/README.md`](../../adr/README.md#mechanics); `scripts/check-docs.sh` (checks 5 and 8,
  `is_inactive_adr`) and `scripts/test-check-docs.sh`.

## Blocked by

- None (can start immediately)

## Mode

with the human: `scripts/check-docs.sh` and its self-test belong to the template, and whether the
check lands there first or here is the maintainer's to decide.
