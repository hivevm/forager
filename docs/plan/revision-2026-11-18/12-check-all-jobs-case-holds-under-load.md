# Ticket: The check-all self-test's case of `CHECK_ALL_JOBS=2` decides the cap under load too

## What to build

The case "CHECK_ALL_JOBS=2 runs no more than two checks at once" of `scripts/test-check-all.sh`
fails when the machine is loaded and passes when it runs alone. The failure was seen in the
sessions of revision-2026-11-18, in `scripts/check-all.sh` itself and nested in the "Normalize map
self-test", which runs every self-test again in a copy of the tree
(`scripts/test-normalize-remap.sh`). A flaky test is a defect
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)), and this one makes a
green run depend on the load.

The case's helper `at_once` appends a stub to every script: it makes a directory under `running/`
named by its process id, writes how many such directories there are, sleeps 0.1 s, and removes
its directory. The case asserts that the highest count is at most 2. Either the runner really
starts a third check while two run, or the measurement counts one that has ended. The runner
counts its jobs itself and waits with `wait -n` (`run` in `scripts/check-all.sh`). Which of the
two it is, nobody has found yet.

The session finds the cause first, from a reproduction under load, and fixes that cause:

- if the runner can exceed the cap, the runner is fixed, and the case keeps its assertion;
- if the measurement is wrong, the measurement is fixed, and the case still fails on a runner
  that ignores `CHECK_ALL_JOBS`.

No retry, longer sleep, or looser bound passes for a fix.

## Acceptance criteria

The load in each criterion is the same: one busy loop per processor,
`for _ in $(seq "$(nproc)"); do yes > /dev/null & done`, running for the whole measurement and
stopped after it.

- [ ] Given that load, on the code before the fix, when the case runs up to 200 times, then it
  fails at least once, and the hand-off records the run count and the failing count: the
  reproduction the fix is measured against.
- [ ] Given that load, after the fix, when the case runs 200 times, then it passes each time.
- [ ] Given the runner's cap removed by hand (every check started at once), when the case runs
  without load, then it fails.
- [ ] Given the cause, when the hand-off is written, then it names the cause, whether the runner or
  the measurement, with the reproduction that showed it.

## Read first

- Touches: `scripts/test-check-all.sh` (`at_once` and the two cases after it),
  `scripts/check-all.sh` (`run`, `settle`, the `wait -n` loop)
- The convention *A test that expects a refusal asserts its reason*, for the case's failure text.

## Blocked by

- None (can start immediately)

## Mode

with the human: no single test decides a timing flake. Criteria 1, 2, and 4 are measurements
under load, which the human judges from the hand-off, so the conditions of
[`AGENTS.md` §4](../../../AGENTS.md#4-working-style) for an unattended run do not hold. The
session works alone up to the hand-off. It stops when 200 runs under the load above do not fail
the case on the code before the fix. It then hands off with what it tried, and does not change the
runner without a cause.
