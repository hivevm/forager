# Ticket: List the commits typed `refactor` that change a test

## What to build

`docs/CONVENTIONS.md` says a commit typed `refactor` changes nothing a test can see. PR #176 typed
b0d30ce and d512e37 `refactor`, though each makes a label style without a transform, or a bound
`shadow_color`, a load error, and their tests changed with them; the review caught it and the
commits stayed as typed. A script can see the cheap half of that: a `refactor` commit whose diff
adds or removes an assertion or a `#[test]`. Write a sensor beside `scripts/sensor-tests-kept.sh`
that lists those commits of a branch for the reviewer and fails nothing, run where the other
sensors run, with a self-test. Once it holds, shorten the convention to what the sensor cannot see.

## Acceptance criteria

- [ ] Given a branch with a `refactor` commit that changes an `assert` in a test, when the sensor
  runs, then it lists that commit and exits 0.
- [ ] Given a branch whose `refactor` commits change no test, when it runs, then it lists none.
- [ ] Given `scripts/check-all.sh`, when it runs, then the sensor's self-test runs with it.

## Read first

- Touches: `scripts/`, `.github/workflows/`, `docs/CONVENTIONS.md`
- `scripts/sensor-tests-kept.sh` and its test, the golden path; `scripts/lib/selftest.sh`;
  `docs/CONVENTIONS.md` (*A script that points git at a repository runs git without git's
  repository variables*, *A self-test asserts what it builds*); CONTRIBUTING.md on sensors.

## Blocked by

- None (can start immediately)

## Mode

unattended: its self-test decides the criteria; a new script and its wiring, one revert; nothing
ADR-0006 keeps with the human. A wrong result shows as a red self-test or a sensor that lists
nothing on PR #176's range. Stop after two failed attempts at the wiring into CI.
