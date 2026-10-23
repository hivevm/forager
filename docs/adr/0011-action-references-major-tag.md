# ADR-0011: Workflows use only `actions/*`, referenced by major version tag, and install the Rust toolchain with rustup

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** every `uses:` line under `.github/workflows/`, `.github/dependabot.yml`, and the `sensors` job of `checks.yml`

## Context

The template references `actions/*` by major version tag. Rust is commonly installed in CI by a
third-party action; `rustup`, preinstalled on the GitHub-hosted runners, reads
`rust-toolchain.toml` itself. CI holds no secret of the project (ADR-0013).

## Decision

We will keep the template's form — only actions of the `actions/` organisation, each by major
version tag, updated by Dependabot — and install the toolchain with `rustup` from
`rust-toolchain.toml` inside `scripts/check-rust.sh` instead of a third-party action.

**Out of scope:** nothing beyond the sentence above.

## Alternatives considered

- **Commit SHAs for every action** — protects against a retagged action, at the cost of an update
  per release; with only `actions/*` and no secrets in CI, the risk does not pay for it.
- **`dtolnay/rust-toolchain` or a similar action** — a third-party action for what rustup does
  from the same file.

## Sources / Prior art

- The catalogue entry *Action references* of the `propose-adr` skill.

## Consequences

- Positive: one source of the toolchain version for container, CI, and local runs.
- Negative / trade-offs: a retagged `actions/*` release reaches CI without a commit; the weekly
  scheduled run surfaces it.
- Follow-ups: revisit when a workflow needs a secret or an action outside `actions/*`.

## Enforcement

`scripts/sensor-action-refs.sh --require major-tag` runs in `scripts/check-all.sh` and in the
`sensors` job of `checks.yml`, beside a comment that cites this ADR with `Verifies:`.
