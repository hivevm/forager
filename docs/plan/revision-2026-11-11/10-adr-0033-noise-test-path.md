# Ticket: Bring ADR-0033's Enforcement to where the noise measurement lies

## What to build

The Enforcement of [ADR-0033](../../adr/0033-noise-measurement.md) (line 253) names the noise
measurement "the ignored test of `crates/cli/tests/msautotest.rs`". Since ca2a20c it lies in
`crates/cli/tests/conformance/noise/`, which carries its `Verifies: ADR-0033`; `msautotest.rs` is
still the test target that holds it. The ADR is accepted and immutable
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 5). PR #176 named the stale path.
Decide with the maintainer: correct it at the next normalization, or supersede ADR-0033 now.
Until then a reader of the Enforcement finds the test by its marker.

## Acceptance criteria

- [ ] Given the maintainer's choice, when it is recorded, then ADR-0033's Enforcement, or its
  successor's, names `crates/cli/tests/conformance/noise/`, or this ticket moves into the
  preconditions of the next normalization.

## Read first

- Touches: `docs/adr/0033-noise-measurement.md`
- `docs/adr/README.md` (mechanics), `.agents/skills/normalize/SKILL.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the ADRs are the maintainer's ([ADR-0006](../../adr/0006-harness.md)).
