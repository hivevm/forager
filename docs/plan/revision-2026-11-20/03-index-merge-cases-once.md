# Ticket: The cases of the ADR index's merge driver stand once

## What to build

`scripts/test-merge-adr-index.sh` is Forager's own self-test of the merge driver of the ADR index.
Four of its six cases, the rows both sides appended, a rule one side changed, a rule both sides
changed, and a line changed below a row, stand again in the template's `scripts/test-git-hooks.sh`,
with the same fixture. The take-up of NUC v0.9.0 had to rework its fixture ADRs from numbers to
identifiers for nothing the template's cases did not already hold. Keep only what the template's
self-test lacks, the merge without the driver and the merge of two ADRs on a copy of the
repository's own files, or fold those into a test of Forager's own beside the template's.

## Acceptance criteria

- [ ] Given the tree, when the self-tests run, then each case of the merge driver runs in exactly
      one of them, and no case of either is lost.
- [ ] Given the driver disabled in `.gitattributes` of a fixture, when the remaining Forager case
      runs, then it still fails as before.

## Read first

- Touches: `scripts/test-merge-adr-index.sh`, `scripts/check-all.sh`,
  `.github/workflows/checks.yml`, `.githooks/merge-adr-index` (its header names both self-tests)
- `scripts/test-git-hooks.sh`, the cases of `indexed`.
- Evidence: the rework of `test-merge-adr-index.sh` in commit 6993013a of PR #222.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides it: the remaining cases run, and a disabled driver still fails one.
- Its scope is one self-test, and a wrong result costs one revert.
- It touches no ADR and nothing OPS-parallel-sessions-capped-at-three keeps with the human.
- A lost case shows as a fall in the count of cases the hand-off states; the revert undoes it.
- Stop: if a case of Forager's has no counterpart and no place in a test of its own, hand off.
