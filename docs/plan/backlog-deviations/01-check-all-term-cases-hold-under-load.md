# Ticket: The check-all self-test's TERM cases pass under load

<!-- Copy to docs/plan/<spec>/<NN>-<slug>.md, next to the spec it slices, if it has one. A
     vertical slice: a narrow but complete path through every layer the change touches, demoable
     or verifiable on its own, sized to one fresh agent session. Claim it by creating the branch
     ticket/<spec>/<NN>-<slug> BEFORE any work, so parallel sessions sharing the repository skip
     it. The branch that lands it deletes this file. Decided in
     docs/adr/0005-feature-layer.md. -->

## What to build

Two cases of `scripts/test-check-all.sh` fail at random on a loaded machine: "and returns once a
check that cleans up on TERM has done so" and "and no check it started outlives it". The backlog
entry `docs/backlog/check-all-term-cases-fail-under-load.md` holds what was seen and the
unconfirmed leads: the cleaning stub's 5 s window before `exec sleep 60`, and the case's 300
polls after which it sends the TERM anyway. Nothing explains a stub that outlives the runner yet.

The session reproduces the failure under a known load first, and fixes the cause the reproduction
shows. If the measurement is wrong, the case still fails on a runner that leaves a check behind.
No retry, longer sleep, or looser bound passes for a fix. The change deletes the backlog entry.

## Acceptance criteria

- [ ] Given a loaded machine, such as one running `scripts/check-all.sh` beside it or `nproc`
  busy loops, when the TERM case runs 50 times, then it passes each time.
- [ ] Given a runner whose `stop` sends no TERM to its checks, changed by hand, when the case
  runs, then it fails with its reason.
- [ ] Given the cause, when the hand-off is written, then it names the cause with the reproduction
  that showed it.

## Read first

- Touches: `scripts/test-check-all.sh` (the fixture `stopped` and the TERM case),
  `scripts/check-all.sh` (`stop`, the subshell's TERM trap in `run`)
- `docs/backlog/check-all-term-cases-fail-under-load.md`; the fix of a7e10c2b and ticket
  revision-2026-11-18/12 (merge d3022202), for the earlier flakes of the same self-test.
- The convention *A test that expects a refusal asserts its reason*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-test decides it, a wrong result costs one revert, and it touches only a
script and its self-test. Stop when no reproduction under load fails the case within 200 runs;
hand off with what was tried, rather than change the runner without a cause.
