# ADR-0013: The project holds no secrets of its own; the baseline of secret scanning with push protection is the only control

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** the repository, its workflows, and their `secrets.*` references

## Context

The specification names credentials as an asset, but they are the users' — connection strings of
mapfiles at runtime — and Q-7 protects them by tests. The project itself needs no token in CI and
publishes nothing yet.

## Decision

We will keep no secret in the repository or its workflows and rely on GitHub secret scanning with
push protection, enabled in the repository settings.

**Out of scope:** the credentials of users' connection strings (Q-7); a token for publishing
crates or releases, decided when a release is automated.

## Alternatives considered

- **A scanner in CI (gitleaks, TruffleHog)** — catches credentials of the project's own shape;
  the project has none, and test fixtures with fake connection strings would need an allow-list.

## Sources / Prior art

- The catalogue entry *Secrets* of the `propose-adr` skill.

## Consequences

- Positive: nothing to rotate, nothing to leak from CI.
- Negative / trade-offs: a leaked credential outside GitHub's patterns goes unnoticed until review.
- Follow-ups: revisit when a workflow needs a secret.

## Enforcement

**Not mechanically decidable:** push protection is a repository setting outside the tree, and a
credential of an unknown shape is recognized only in review. ADR-0011's action-reference check
and the absence of `secrets.` in the workflows can be read by a reviewer in one grep.
