# Ticket: Give the noise measurement a module of its own and one way out of the push checks

## What to build

`scripts/check-rust.sh` runs the ignored tests of `msautotest` with `--skip
mapserver_renders_the_cases_in_scope_within_its_noise`, and `.github/workflows/noise.yml` runs the
same name with `--exact`: renamed, the test runs on every push and fails there without `map2img`
and the label reader, and a second ignored test that needs MapServer joins the push checks
unnoticed. Its code (`measure`, `reproduced`, `noise_report`, and `read_labels`, which holds the
reader's labels to the records) sits in the middle of `msautotest.rs` beside the G-1 loading, the
G-3 round trip, and the conformance render, where `conformance/` and `metric/` are modules, and it
shares the metric lookup (`metric_of`) with the harness. Move it into a `noise/` module, and keep it
out of the push checks by one rule a test holds, such as skipping itself without
`FORAGER_MAP2IMG` and `FORAGER_MAPSERVER_LABELS`, instead of a name written twice
([ADR-0033](../../adr/0033-noise-measurement.md)).

## Acceptance criteria

- [ ] Given `scripts/check-rust.sh`, when it runs without `FORAGER_MAP2IMG` and
  `FORAGER_MAPSERVER_LABELS`, then the noise test does not run, whatever it is named, and no
  `--skip` names it.
- [ ] Given the noise workflow, when it runs, then it measures as before.

## Read first

- [ADR-0033](../../adr/0033-noise-measurement.md); `crates/cli/tests/msautotest.rs`;
  `scripts/check-rust.sh`; `.github/workflows/noise.yml`.

## Blocked by

- None (can start immediately)

## Mode

unattended: `scripts/check-rust.sh` and a run of the noise workflow decides its criteria; its scope
is the noise measurement's code and its two callers, and a wrong result costs one revert; it touches
none of the parts [ADR-0006](../../adr/0006-harness.md) keeps with the maintainer and counts against
its cap of three; a wrong result shows as a red push check or a noise run that measures nothing;
stop: after two attempts that leave `scripts/check-all.sh` red, or when the noise workflow cannot be
run to confirm it, the session leaves its state in this file and hands off.

## Worker's findings (2026-11-10)

Implemented on `work/group-1-2026-11-10`; the stop "when the noise workflow cannot be run to
confirm it" is reached, since the session may not push, so this file stays until a run confirms it.

- **Where the module lies.** It is `crates/cli/tests/conformance/noise/` (`mod.rs`, `tests.rs`), not
  a top-level `noise/`: ADR-0033's *When* lists the paths a push triggers the workflow on, and
  `scripts/test-build-mapserver.sh` holds the trigger to that list line by line. A top-level
  `noise/` would need a new trigger path, which only a superseding ADR can add; under
  `conformance/` the trigger `crates/cli/tests/conformance/**` covers it unchanged.
- **The one rule.** `noise::mapserver` decides from `FORAGER_MAP2IMG` and
  `FORAGER_MAPSERVER_LABELS`: neither set skips the measurement, both naming a file runs it, one
  missing or empty fails it. The unit test
  `the_noise_measurement_runs_only_where_mapserver_is_named` holds it. `scripts/check-rust.sh` no
  longer passes `--skip`; the workflow runs `--ignored conformance::noise::` and fails when its job
  summary holds no report, so a test renamed out of the module does not pass unmeasured.
- **Confirmed locally.** Without the variables the ignored test passes after printing that it
  skipped; with both set to `/bin/false`, as a stand-in for a build, it measures and fails on all
  74 cases with `map2img fails` and writes the report to `GITHUB_STEP_SUMMARY`.
- **Left to confirm.** One run of `.github/workflows/noise.yml` on the branch's push, which the
  change triggers through `crates/cli/tests/conformance/**`, must measure as before. Delete this
  file once it does.
