# Ticket: Take up the template release that binds a review to the commit it read

## What to build

In #155 the fix commits that answered a review were about to merge without a second review; the
session's tool permissions refused the merge, not anything in the repository. Nothing here ties
the *Review* section of a pull request to the commit the reviewer read: the `review` skill and
[ADR-0007](../../adr/0007-harness.md) say a change is reviewed in a fresh session, and say nothing
of the commits after it. This repository carries NUC v0.2.0 (`README.md`, Template). NUC's later
releases add a `Reviewed at:` line to the pull request's review and a sensor that reports the
commits after it to the reviewer. Take up the release that carries it, under
[ADR-0005](../../adr/0005-template-releases.md), or, should the maintainer not take it up yet, ask
upstream for that part alone.

## Acceptance criteria

- [ ] Given a branch with a commit after the one its review names, when the sensors run, then
  the review-binding sensor names that commit.
- [ ] Given the take-up, when it lands, then the Template line of `README.md` names the release
  taken up.

## Read first

- ADR-0005, ADR-0007; the `review` and `work-tickets` skills; the release notes of the NUC
  release that adds the review binding.

## Blocked by

- None (can start immediately)

## Mode

with the human: a template release is taken up by its maintainer
([ADR-0005](../../adr/0005-template-releases.md)), and it changes the skills and `AGENTS.md`.
