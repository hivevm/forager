# Ticket: The pre-push hook clears git's repository variables before it runs the checks

## What to build

In a linked worktree (`git worktree add`, as the work-tickets skill makes one per ticket), git
runs the pre-push hook with `GIT_DIR` and the other repository variables set. The hook passes them
on to `scripts/check-all.sh`, and every git command of the checks then acts on the real
repository instead of its own fixture: the hook self-test (`scripts/test-git-hooks.sh`) set
`core.bare=true` and `core.hooksPath` in `.git/config`, committed and created branches there, and
pushed its fixture branch `feature` to GitHub; `scripts/fetch-msautotest.sh` set a sparse checkout
on the worktree. This happened on 2026-10-24 with the push of
`ticket/ir-completeness/10-layer-projection-and-scale-ranges`, and before in hivevm/comb.

The pre-push hook unsets every variable `git rev-parse --local-env-vars` names before it runs the
checks, as the template's fix does (hivevm/nuc, branch `fix/hook-git-env`), and the hook
self-test gains a case that pushes from a linked worktree and asserts that the repository's
config and refs are unchanged.

## Acceptance criteria

- [ ] Given a fixture repository with a linked worktree and a check-all that runs git commands,
  when a push runs from the worktree, then the fixture's `.git/config`, its branches, and its
  remote are unchanged, and the push proceeds when the checks are green.
- [ ] Given the existing cases of `scripts/test-git-hooks.sh`, when it runs, then they pass.

## Read first

- `.githooks/pre-push`, `scripts/test-git-hooks.sh`; `git help githooks` on the environment of
  a hook; [ADR-0007](../../adr/0007-harness.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes a git hook, which the harness keeps with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
