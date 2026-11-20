# OPS-actions-by-major-tag: Workflows reference actions by major version tag, as the template ships them

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** every `uses:` line under `.github/workflows/`, `.github/dependabot.yml`, the `sensors` job of `.github/workflows/checks.yml`, and the sensor of the action references `scripts/sensor-action-refs.sh` with its self-test `scripts/test-sensor-action-refs.sh`

## Context

The template references `actions/*` by major version tag, updated by Dependabot; CI holds no secret
of the project ([SEC-no-secrets-of-its-own](SEC-no-secrets-of-its-own.md)).

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
`sensors` job of `.github/workflows/checks.yml`, each beside a comment that cites this ADR with
`Verifies:`.
