# The fixture git self-test runs every self-test a second time, half of a run of the checks

**Observed:** ticket revision-2026-11-20/04 (2026-11-21), stopped at its stop. The maintainer
moved it back to the backlog until the run time of the checks gets in the way.

`scripts/test-fixture-git-env.sh` holds every script that points git at a directory to the
`GIT_DIR` guard of `docs/CONVENTIONS.md`. It runs every self-test a second time, one after the
other, with `GIT_DIR` set. On a shared machine of 24 processors it took 141 s of a run of 272 s
of `scripts/check-all.sh`, 66 cases. The fixture itself is cheap. In one sequential pass the
slowest self-tests were `test-check-all.sh` 67 s, `test-check-docs.sh` 59 s,
`test-check-shell.sh` 39 s, and `test-sensor-binding-adrs.sh` 34 s.

A parallel run, one decoy per self-test, held all 66 cases and every failure by name. It took
85 s to 107 s inside `check-all.sh`, not half, and it slowed the checks beside it: the Rust step
went from 130 s to 244 s and 305 s. No parallel run is faster than its slowest self-test, and
`test-check-all.sh` alone took about 70 s under that load. The patch was not committed.

**Where:** `scripts/test-fixture-git-env.sh`, `scripts/check-all.sh`,
`.github/workflows/checks.yml`; the entry on a caller's `GIT_DIR` in `docs/CONVENTIONS.md`.

**What a change weighs:**

- **One pass with `GIT_DIR` on a decoy**: `check-all.sh` and CI run their one pass of the
  self-tests with `GIT_DIR` naming a decoy, and the second pass goes. It holds the same scripts
  at no extra time, and it changes `check-all.sh` and the CI workflow.
- **The parallel run**: about a third saved on one machine, untested on CI's runners of two
  processors, at a cost to the checks that run beside it.
- **Leave it**: no work while the run time does not get in the way.
