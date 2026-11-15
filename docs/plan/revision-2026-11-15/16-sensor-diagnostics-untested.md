# Ticket: A sensor that lists the diagnostic codes no test asserts

## What to build

The warning `horizon-points-limit` came into the engine without a test that pushes it
(reprojection review 05-07, S4). Its first fix tested a helper, not that the render pushes it
(review 04, #6). Only 9a2b860c tested it through a render. Diagnostic codes are stable and
tests match on them (`docs/CONVENTIONS.md`), so a sensor can read them. It lists each code written
as a string literal into `Diagnostic::warning` or `Diagnostic::error` under `crates/*/src`, then
names each code that no file under a crate's `tests/`, `src/tests.rs`, or a `#[cfg(test)]`
module names. It reports and fails nothing (ADR-0005). `scripts/check-all.sh` runs it, and the
review skill already hands every sensor's output to the reviewer.

## Acceptance criteria

- [ ] Given a fixture crate with a code no test names, when the sensor's self-test runs it, then
  it names that code and its file.
- [ ] Given a fixture where a test names the code, when the self-test runs, then the sensor names
  nothing.
- [ ] Given the tree, when `scripts/check-all.sh --verbose` runs, then the sensor's report is
  printed and the run's result does not depend on it.

## Read first

- Touches: `scripts/sensor-diagnostics-untested.sh`, its self-test `scripts/test-*.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml` (the sensors job),
  `docs/adr/README.md` only if the index names sensors
- `docs/adr/0007-rust-toolchain.md` (every sensor `check-all.sh` runs),
  `docs/adr/0005-feature-layer.md`; `scripts/lib/expect.sh`, `scripts/lib/selftest.sh`.

## Blocked by

- docs/plan/revision-2026-11-15/14-scripts-and-headers.md

## Mode

unattended: the self-test decides it; one revert. Stop when the codes cannot be read without a Rust
parser. That is a judgement on the sensor's worth, for the maintainer. It touches nothing ADR-0006
keeps with the human, and it counts against ADR-0006's cap of three open agent branches.
