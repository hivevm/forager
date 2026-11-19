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
scheduled run is no fault of the branch being checked, so it fails nothing. It reads what only
the GitHub repository holds, so ADR-0002's clause *Sensors of the GitHub repository* binds it: it
runs on a human's instruction, not in `scripts/check-all.sh` or `checks.yml`, and its self-test
runs like any other.

The sensor, `scripts/sensor-scheduled-runs.sh`, asks `gh` for the latest scheduled run of each
workflow on `main`. It lists each one that did not succeed, with its workflow, date, and URL.
Without `gh`, or without its authentication, it says so and exits as
`scripts/sensor-repo-settings.sh` does. `scripts/check-all.sh` lists it under the comment of the
sensors run on instruction, and runs its self-test, which puts a stand-in `gh` first on `PATH`.
That a human seldom gives the instruction is the gap this leaves. Who runs it, and when, is
the human's to say: the hand-off names the command and asks.

## Acceptance criteria

- [ ] Given a stand-in `gh` that answers with a failed latest scheduled run of Fuzz and a
  successful one of Checks, when the self-test runs the sensor, then it lists Fuzz with its date
  and URL, does not list Checks, and exits 0.
- [ ] Given a stand-in `gh` that answers with an older failed run of a workflow and a later
  successful one, when the sensor runs, then it lists nothing for that workflow.
- [ ] Given no `gh` on `PATH`, or a `gh` that is not authenticated, when the sensor runs, then it
  says that it could not ask, with the exit code that `sensor-repo-settings.sh` gives there.
- [ ] Given the tree after the change, when `scripts/check-all.sh` runs, then the self-test runs
  and the sensor does not, the sensor is named under the comment of the sensors on instruction,
  and the check passes.

## Read first

- Touches: `scripts/sensor-scheduled-runs.sh`, `scripts/test-sensor-scheduled-runs.sh`,
  `scripts/check-all.sh`
- `docs/adr/0002-decisions-verified-by-tests.md`, the clause *Sensors of the GitHub repository*
- `scripts/sensor-repo-settings.sh` and its self-test: the precedent for a sensor that asks
  `gh`, for a stand-in `gh`, and for its place in `scripts/check-all.sh`
- `.github/workflows/checks.yml`, `fuzz.yml`, `mutants.yml` (their `schedule:`)
- `docs/CONVENTIONS.md`: *A self-test asserts what it builds*; *A script that points git at a
  repository runs git without git's repository variables*

## Blocked by

- None (can start immediately)

## Mode

unattended. The self-test decides it with a stand-in `gh`, and a wrong result costs one revert.
The session runs no `gh` of its own: the sensor itself runs only on the human's instruction.
Stop if `gh` cannot be asked for a workflow's latest scheduled run on a branch without listing
every run. Hand off with the
`gh` command that would do it.
