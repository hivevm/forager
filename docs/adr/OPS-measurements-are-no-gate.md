# OPS-measurements-are-no-gate: A workflow that measures on a schedule is no gate, and a script only it runs stands on an `# On instruction:` line

- **Status:** 🟡 proposed
- **Date:** 2026-11-20
- **Deciders:** Markus Brigl
- **Applies to:** `.github/workflows/fuzz.yml`, `.github/workflows/mutants.yml`, `.github/workflows/noise.yml`, the `# Not a gate:` and `# On instruction:` lines of `scripts/check-all.sh`, and the rule of those lines in `scripts/check-docs.sh`
- **Extends:** NUC-checks-are-bash §5

## Context

[NUC-checks-are-bash §5](NUC-checks-are-bash.md) has every check run by `scripts/check-all.sh` and
by CI, and spares only a workflow that publishes rather than checks. Forager measures on a
schedule: the fuzz run of Q-3, the mutation testing of
[OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md), and the noise measurement
of [TST-noise-measured-against-mapserver](TST-noise-measured-against-mapserver.md). Each runs for
minutes to hours, and a push that waited for one would wait for a measurement, not a check. The
decision was a clause of Forager's ADR on tests until the migration to NUC v0.9.0.

## Decision

We will treat a scheduled measurement like a publishing workflow:

1. **No gate.** A workflow that measures on a schedule is no gate. `scripts/check-all.sh` names it
   on a `# Not a gate:` line, and its jobs may run commands of their own.
2. **Its scripts.** A script that only such a workflow runs, or only another script, stands on an
   `# On instruction:` line of `scripts/check-all.sh` instead of running in it. A push then does
   not wait for a measurement.

**Out of scope:** what each measurement reports, which the ADR that adds it decides.

## Alternatives considered

- **Run every script in `scripts/check-all.sh`** — a push would wait for hours of fuzzing and
  mutation.
- **Leave the scripts unlisted** — the rule of the lists would fail on a script nobody runs.

## Sources / Prior art

None — an extension of the template's rule to the measurements Forager runs.

## Consequences

- Positive: a push waits only for checks; the measurements run where their time is spent.
- Negative / trade-offs: a red measurement reaches no pull request; the scheduled runs sensor
  reports it on instruction.

## Enforcement

The rule of the `# Not a gate:` and `# On instruction:` lines in
[`scripts/check-docs.sh`](../../scripts/check-docs.sh) holds both clauses, and its self-test,
`scripts/test-check-docs.sh`, decides both sides and cites this ADR.

**Not mechanically decidable:** whether a new workflow measures or checks, which the review of the
change that adds it decides.
