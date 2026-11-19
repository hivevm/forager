# Ticket: A sensor lists the scheduled workflow runs on main that failed

## What to build

Three workflows run on a schedule on `main`: Checks, Fuzz, and Mutants. Their results reach no
one. A pull request shows only the checks of its own head, and no sensor reads a scheduled run.
In the four weeks up to this revision:

- Fuzz failed on 2026-10-25, 11-01, 11-08, and 11-15, with two panics and two timeouts of the
  render target (ticket 05);
- Mutants failed on all four dates, before its first mutant (ticket 04);
- Checks failed on 2026-11-16, on draft ADRs that had reached `main`, and passed again after their
  numbering.

So Q-3's measure and ADR-0007's mutation testing reported nothing for a month. A check could
have caught it, so it becomes one
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)). It is a sensor: a red
scheduled run is no fault of the branch being checked, so it fails nothing.

The sensor, `scripts/sensor-scheduled-runs.sh`, asks `gh` for the latest scheduled run of each
workflow on `main`. It lists each one that did not succeed, with its workflow, date, and URL.
Without `gh`, or without its authentication, it says so, reports nothing, and exits 0. It runs in
`scripts/check-all.sh` among the sensors, with a self-test that puts a stand-in `gh` first on
`PATH`. `scripts/sensor-repo-settings.sh` runs only on instruction, because it fails without
`gh`. This sensor stands down instead, so that every session and every pull request sees it.

## Acceptance criteria

- [ ] Given a stand-in `gh` that answers with a failed latest scheduled run of Fuzz and a
  successful one of Checks, when the self-test runs the sensor, then it lists Fuzz with its date
  and URL, does not list Checks, and exits 0.
- [ ] Given a stand-in `gh` that answers with an older failed run of a workflow and a later
  successful one, when the sensor runs, then it lists nothing for that workflow.
- [ ] Given no `gh` on `PATH`, or a `gh` that is not authenticated, when the sensor runs, then it
  says that it could not ask and exits 0.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then the sensor and its
  self-test run, and the check passes.

## Read first

- Touches: `scripts/sensor-scheduled-runs.sh`, `scripts/test-sensor-scheduled-runs.sh`,
  `scripts/check-all.sh`
- `scripts/sensor-repo-settings.sh` and its self-test: the precedent for a sensor that asks
  `gh` and for a stand-in `gh`
- `.github/workflows/checks.yml`, `fuzz.yml`, `mutants.yml` (their `schedule:`)
- `docs/CONVENTIONS.md`: *A self-test asserts what it builds*; *A script that points git at a
  repository runs git without git's repository variables*

## Blocked by

- None (can start immediately)

## Mode

unattended. The self-test decides it with a stand-in `gh`, and a wrong result costs one revert.
The session runs no `gh` of its own, so it needs no instruction. Stop if `gh` cannot be asked for
a workflow's latest scheduled run on a branch without listing every run. Hand off with the
`gh` command that would do it.
