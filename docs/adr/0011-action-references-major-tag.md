# ADR-0011: Workflows reference actions by major version tag, as the template ships them

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** every `uses:` line under `.github/workflows/`, `.github/dependabot.yml`, and the `sensors` job of `checks.yml`

## Context

The template references `actions/*` by major version tag, updated by Dependabot; CI holds no secret
of the project ([ADR-0013](0013-no-project-secrets.md)).

## Decision

We will keep the template's form and reference every action by its major version tag.

**Out of scope:** nothing beyond the sentence above.

## Alternatives considered

None — cheap to reverse.

## Sources / Prior art

None — cheap to reverse.

## Consequences

None — cheap to reverse.

## Enforcement

`scripts/sensor-action-refs.sh --require major-tag` runs in `scripts/check-all.sh` and in the
`sensors` job of `checks.yml`, beside a comment that cites this ADR with `Verifies:`.
