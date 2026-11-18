# Ticket: A check or a sensor sees a commit of a branch on which the fast checks fail

## What to build

Four branches of this revision's scope carried commits on which a check fails, between a green
base and a green head:

- a53e7ad7 (ticket 06 of revision-2026-11-16) fails `scripts/check-notice.sh`: `circle.rs` lacks
  MapServer's notice until a9512cb6;
- f2731b6c (ticket 11 of revision-2026-11-18) fails clippy;
- c7b0dabb up to 8b619a1c (ticket 09 of revision-2026-11-18) fail the shell lint;
- 399ac7b8 (ticket 02 of revision-2026-11-18) fails rustfmt, fixed by 2b48ccd0 after it.

Each review named it, and none was amended, since a reviewed commit is not rewritten. Each one
breaks a bisect through the branch. The hooks run `scripts/check-all.sh` at the head before a push
and on a merge's resolution (`.githooks/pre-push`, `.githooks/pre-commit`). Nothing looks at the
commits between. No rule asks every commit of a branch to be green.
[`AGENTS.md` §4](../../../AGENTS.md#4-working-style) asks it only of the steps of a wide mechanical
refactor.

A check could catch it, so the design revision files it as one
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)). Whether it becomes a
check is the human's decision. A check that refuses would make a rule of every commit being green.
Only [`AGENTS.md` §6](../../../AGENTS.md#6-project-rules) and the hooks can state that rule, and it
costs time on every push. The forms to weigh with the human:

- **A sensor**, which fails nothing. It runs the fast checks on each commit of the branch since
  `main`, and lists those that fail for the reviewer: rustfmt, the shell lint, `check-notice.sh`,
  and `check-docs.sh`, without the build and test. It is a sensor like `sensor-refactor-tests.sh`.
- **A check in the pre-push hook**, which refuses the push while a commit since `main` fails the
  fast checks. That needs a rule in `AGENTS.md` and an entry under Git hooks in `CONTRIBUTING.md`.
  It also needs a way out for a reviewed commit that may not be rewritten.
- **Nothing**: the review keeps naming it, and a bisect skips such a commit.

The refusing check with its rule would set the cost of every push and constrain the toolchain's
checks. That is a decision for an ADR, revising or superseding ADR-0007, the toolchain and its
checks. So that form goes through the `propose-adr` skill as a `proposed` ADR before anything is
built, per [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules). The sensor fails nothing and needs no
ADR.

## Acceptance criteria

- [ ] Given the human's choice among the forms, when the ticket is worked, then the hand-off
  names the choice and who made it.
- [ ] Given a fixture branch with a commit that fails rustfmt between a green base and a green
  head, when the chosen sensor or check runs, then it names that commit and the check it fails.
  For the "nothing" form this criterion does not apply, and the ticket file is deleted with the
  choice recorded.
- [ ] Given this branch's history, when the chosen sensor or check runs, then it takes no longer
  than the human accepts, measured and named in the hand-off.

## Read first

- Touches: a new `scripts/sensor-*.sh` and its self-test, or `.githooks/pre-push` and
  `scripts/test-git-hooks.sh`; `scripts/check-all.sh`; for a check,
  [`AGENTS.md` §6](../../../AGENTS.md#6-project-rules) and `CONTRIBUTING.md` (Git hooks)
- ADR-0001 (a rule is enforced by what every agent hits), ADR-0007; the merge messages of f63b9cd3,
  04fea3ec, b06eb67d, and a6f0c9f6, their *Doubts* and *Review*.

## Blocked by

- None (can start immediately)

## Mode

with the human: the choice between a sensor, a refusing check, and nothing sets a rule of the
project and the cost of every push. A rule in `AGENTS.md` is the human's.
