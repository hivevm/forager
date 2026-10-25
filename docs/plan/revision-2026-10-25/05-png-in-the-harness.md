# Ticket: Settle what ADR-0017 says about the harness decoding references with `png`

## What to build

[ADR-0017](../../adr/0017-png-crate.md) decides "We will encode and decode PNG with the image-rs
`png` crate in `encode`", and its *Applies to* names `crates/encode` alone. Since #76 the CLI's
integration tests decode the msautotest reference images with `png` as a dev-dependency
(`crates/cli/tests/metric/`), as the conformance harness spec decided: a reference of any colour
type is expanded to RGBA by the `png` crate in the harness, while the encoder's decoding stays
the strict one G-12 needs. #76 raised it as doubt 1. A reader of ADR-0017 alone takes the second
use for a violation. Two ways out, for the maintainer:

1. Rule 2 of [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules) needs no ADR for tests and
   dev-only tooling, so the harness's use stands; the CLI's manifest comment already says why. Then
   nothing changes but the version: `png = "0.18.1"` is written in two manifests and may drift.
2. A short ADR superseding ADR-0017, through `propose-adr`, that names both uses and widens
   *Applies to* to the harness.

Either way, `png` moves to `[workspace.dependencies]` in `Cargo.toml`, so that the encoder and the
harness read one version and a Dependabot bump moves both.

## Acceptance criteria

- [ ] Given the maintainer's choice, when it lands, then either a `proposed` ADR superseding
  ADR-0017 is in an ADR-only pull request, or the pull request says that rule 2 covers the use.
- [ ] Given `Cargo.toml`, when it is read, then `png` is a workspace dependency and both
  `crates/encode` and `crates/cli` take it with `workspace = true`; `cargo tree -d` shows one
  `png`.

## Read first

- [ADR-0017](../../adr/0017-png-crate.md), [ADR-0012](../../adr/0012-crate-supply-chain.md); the
  *Metric* decision of `docs/plan/conformance-harness/spec.md`; the PR body of #76.

## Blocked by

- None (can start immediately)

## Mode

with the human: whether an accepted ADR is superseded is the maintainer's decision.
