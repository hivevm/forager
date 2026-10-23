# ADR-0013: The project holds no secrets of its own, and secret scanning with push protection is the only control

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** the repository, its workflows, and their `secrets.*` references

## Context

The specification names credentials as an asset, but they are the users' — connection strings of
mapfiles at runtime — and Q-7 protects them by tests. The project itself needs no token in CI.

## Decision

We will keep no secret in the repository or its workflows and rely on GitHub secret scanning with
push protection, enabled in the repository settings.

**Out of scope:** the credentials in users' connection strings (Q-7); a token for publishing,
decided when a release is automated.

## Alternatives considered

None — cheap to reverse.

## Sources / Prior art

None — cheap to reverse.

## Consequences

None — cheap to reverse.

## Enforcement

**Not mechanically decidable:** push protection is a repository setting outside the tree, and a
credential of an unknown shape is recognized only in review.
