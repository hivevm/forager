# OPS-backlog-commits-are-sensed: A sensor lists each commit that adds a backlog entry beside a change outside the backlog

- **Status:** 🟡 proposed
- **Date:** 2026-11-20
- **Deciders:** Markus Brigl
- **Applies to:** `scripts/sensor-backlog-commits.sh` and its self-test
- **Extends:** NUC-large-work-is-planned §7

## Context

[NUC-large-work-is-planned §7](NUC-large-work-is-planned.md) has a session create each backlog entry
in a commit of its own, and `scripts/sensor-backlog.sh` lists the entries. Nothing in the template
tells a reviewer when an entry rode along in a commit with other work, which a later revert or
cherry-pick then carries with it. The decision was a clause of Forager's ADR on planning until the
migration to NUC v0.9.0.

## Decision

We will sense the rule of the backlog's own commit:

1. **Sensor.** `scripts/sensor-backlog-commits.sh` lists each commit since the base that adds a
   file under `docs/backlog/` beside a change outside it. It fails nothing; the reviewer asks for
   the split.

**Out of scope:** the backlog's form and triage, which
[NUC-large-work-is-planned](NUC-large-work-is-planned.md) decides.

## Alternatives considered

- **A check that fails** — a commit already on a branch can only be split by a rebase, which a
  sensor leaves to the reviewer's judgement.
- **Review alone** — the commit an entry rode in is easy to miss in a long branch.

## Sources / Prior art

None — an enforcement of the template's rule.

## Consequences

- Positive: an entry that rode along is named before the branch merges.
- Negative / trade-offs: one more sensor in the sensors job.

## Enforcement

`scripts/sensor-backlog-commits.sh` runs in the sensors job of
[`checks.yml`](../../.github/workflows/checks.yml) and in `scripts/check-all.sh`; its self-test,
`scripts/test-sensor-backlog-commits.sh`, cites this ADR.

**Not mechanically decidable:** whether a listed commit is to be split, which the reviewer decides.
