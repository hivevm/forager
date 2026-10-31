# Ticket: Report a change of an ADR's body in a pull request that is not ADR-only

## What to build

#156 changed the *Box* clause of proposed
[ADR-0029](../../adr/0029-label-positions-from-mapservers-label-cache.md) (a drawn member's box
holds its `labelpoint` markers) in its implementation pull request. The correction was right, but
[`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rules 3 and 4, submit an ADR and return a finding
on a proposed ADR as a revision in an ADR-only pull request, so that it is decided as an ADR and not
reviewed as the ticket's means; the ADR's other open revision,
`docs/plan/revision-2026-10-31b/09-adr-0029-revised-to-the-reader-built.md`, now starts from a body
no ADR pull request showed. Nothing reports it: a sensor could name each ADR under `docs/adr/` whose
lines other than `Status` and `Deciders`, the two a human may edit (rule 5), change in a diff that
also changes files outside `docs/`. The rule and the sensors are the template's, so the change is
raised there first; the maintainer may also judge that a factual correction of a proposed ADR may
travel with its implementation, which is a change of the rule.

## Acceptance criteria

- [ ] Given a fixture branch that changes an ADR's *Context* and a source file, when the sensor
  runs, then it names the ADR, and fails nothing.
- [ ] Given a branch that changes only an ADR's `Status` or `Deciders` line with its
  implementation, when the sensor runs, then it names nothing.
- [ ] Given the ticket's pull request, when it merges, then it leaves the drafted sensor and its
  self-test in the tree, with its description naming what the template's maintainer raises
  upstream.

## Read first

- [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules);
  [ADR-0002](../../adr/0002-decisions-verified-by-tests.md)
  (checks and sensors); [ADR-0005](../../adr/0005-template-releases.md); `scripts/check-docs.sh`;
  commit `f7c3ae3` of #156.

## Blocked by

- None (can start immediately)

## Mode

with the human: the rule and the sensors are the template's, changed upstream by its maintainer.
