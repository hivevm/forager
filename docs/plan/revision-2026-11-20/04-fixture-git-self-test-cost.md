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
