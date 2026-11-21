# Ticket: The fixture git self-test costs less than half of the run of the checks

## What to build

`scripts/test-fixture-git-env.sh` holds every script that points git at a directory to the
`GIT_DIR` guard of `docs/CONVENTIONS.md`. In the last run of `scripts/check-all.sh` of PR #222 it
was the slowest check, 132 s of a run of 244 s. It earned its place: it found the three sensors
NUC v0.10.0 brought without the guard. Find what its time goes to, and cut it without dropping a
script it holds, for instance by running each script's case in parallel or by a cheaper fixture.

## Acceptance criteria

- [ ] Given the tree, when `scripts/check-all.sh` runs, then the fixture git self-test takes less
      than half of its present 132 s on the same machine, and the hand-off states both times.
- [ ] Given a script that points git at a directory without the guard, when the self-test runs,
      then it still fails naming the script.
- [ ] Given the tree, when the self-test runs, then it holds as many scripts as before.

## Read first

- Touches: `scripts/test-fixture-git-env.sh`
- `docs/CONVENTIONS.md`, the entry on a caller's `GIT_DIR`.
- The times `scripts/check-all.sh` prints at the end of its run.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides it: the time `check-all.sh` prints, and the self-test's own cases.
- Its scope is one self-test, and a wrong result costs one revert.
- It touches no ADR and nothing OPS-parallel-sessions-capped-at-three keeps with the human.
- A dropped script shows as a fall in the count of scripts held; the revert undoes it.
- Stop: after two approaches that do not reach half the time, hand off with their measurements.

## Where it stands (2026-11-21, handed off at the stop)

Measured on one machine of 24 processors that other sessions shared, at a load average of 20 to
40. The run-to-run noise is large: one sequential pass of the self-tests took 149 s, a second 280 s.

- Baseline, unchanged tree: `scripts/check-all.sh` printed the self-test at 141 s of a run of
  272 s, 66 cases. Alone it took 149 s.
- Where the time goes: the fixture is cheap. The time is the self-tests it runs a second time,
  one after the other, with `GIT_DIR` set. In one sequential pass the slowest were
  `test-check-all.sh` 67 s, `test-check-docs.sh` 59 s, `test-check-shell.sh` 39 s, and
  `test-sensor-binding-adrs.sh` 34 s. Alone, `test-check-all.sh` took 30 s and
  `test-check-docs.sh` 33 s, with `GIT_DIR` set or not.
- Approach 1: each self-test got a decoy of its own, a copy of one made at the start, and the
  self-tests ran in parallel, one per processor. The verdicts were then read in order. It still
  held 66 cases. It still failed by name on a script without the guard, on a self-test that
  failed, and on one that wrote to the decoy. Alone it took 47 s. In `check-all.sh` it took
  107 s (run 427 s) and then 85 s (run 341 s), not under the 70.5 s needed. In those runs the
  Rust step went from 130 s to 305 s and 244 s, and `test-check-all.sh` from 25 s to 90 s and
  70 s. So the parallel runs add their load to the checks that run beside them.
- Why a second approach inside this one file does not reach it: a parallel run takes no less than
  its slowest self-test. Under the load of `check-all.sh`, `test-check-all.sh` alone took about
  70 s, which is the target. A cheaper fixture saves nothing, because the fixture is not where the
  time goes. Running the slow self-tests first changes little, because they start at once already.
- What would reach it, and is outside this ticket: `check-all.sh` and CI already run every
  self-test once. Running that one pass with `GIT_DIR` naming a decoy would hold the same
  scripts and drop the second run. That changes `scripts/check-all.sh` and the CI workflow, so
  it needs the maintainer's decision. The patch of approach 1 is under `.scratch/`. It is not
  committed.
