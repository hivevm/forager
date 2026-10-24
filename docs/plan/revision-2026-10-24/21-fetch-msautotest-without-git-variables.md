# Ticket: The msautotest fetch runs its git without git's repository variables

## What to build

`scripts/fetch-msautotest.sh` drives git on a repository of its own, the sparse checkout under
`target/msautotest/`, and does not clear git's repository variables, which the convention *A
test's fixture repository runs git without git's repository variables* asks of every script that
does so. `scripts/check-all.sh` and the pre-push hook clear them before they call it, so today
only a direct call is exposed. Run with `GIT_DIR` naming a decoy repository and no checkout yet,
its git commands act on the decoy instead of the checkout: it reads the decoy's `HEAD` as the
checkout's and fails with "the checkout is <decoy commit>, the pin is 7515ccc…" (verified on
2084f78). The decoy self-test `scripts/test-fixture-git-env.sh` covers only `scripts/test-*.sh`
and so did not see it.

The script clears the variables as the self-tests do, and the decoy self-test, or a check beside
it, covers every script under `scripts/` that runs git with `-C` or `clone`, not only the
self-tests.

## Acceptance criteria

- [ ] Given `GIT_DIR` naming a decoy repository and no checkout under `target/msautotest/`, when
  `scripts/fetch-msautotest.sh` runs, then it prints the suite's directory and the decoy's refs,
  config, and objects are unchanged.
- [ ] Given a script under `scripts/` that runs `git -C` on a directory of its own without
  clearing the variables, when the self-test runs, then it fails naming that script.

## Read first

- The convention on fixture repositories in `docs/CONVENTIONS.md`;
  [ADR-0002](../../adr/0002-decisions-verified-by-tests.md).
- `scripts/fetch-msautotest.sh`, `scripts/test-fixture-git-env.sh`, `scripts/check-all.sh`.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- a test decides it: the decoy self-test, red before the fix and green after;
- its scope is `scripts/` alone, and a wrong result costs one revert;
- it touches no part ADR-0007 keeps with the human, and counts toward its cap of three;
- a wrong result shows as a red `shell` or `rust` job, or a fetch that fails in CI; it is undone
  by reverting the commit;
- stop: when the fetch cannot be run under a decoy without the network after two attempts, leave
  the static check, state the gap here, and hand off.
