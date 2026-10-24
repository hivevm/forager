# Ticket: Run every fixture repository's git without git's repository variables

## What to build

PR #40 made the pre-push hook clear what `git rev-parse --local-env-vars` names, after a push
from a linked worktree passed `GIT_DIR` to the self-tests, which then wrote into the real
repository and pushed a fixture branch. The fixtures themselves still inherit the variables:
`a_release_tag_from_before_the_schema_holds_no_schema_file` in
`crates/style-ir/tests/schema.rs` runs `git init`, `commit`, and `tag` with `git -C <temp dir>`,
and `-C` does not override `GIT_DIR`; run under a `GIT_DIR`, it commits to the real repository
and tags it `v0.1.0` with a fixture `schema/style-document-1.0.json`, which the schema check
would then take for a frozen release. `scripts/test-sensor-plan-frontier.sh` and
`scripts/test-git-hooks.sh` build fixtures the same way.

Each fixture clears those variables for its own git (`Command::env_remove` in Rust, `env -u` or
`unset` in bash), as the new convention in `docs/CONVENTIONS.md` asks, and
`scripts/check-all.sh` clears them at its start, so that every caller is covered and not only the
hook.

## Acceptance criteria

- [ ] Given `GIT_DIR` set to a decoy repository, when the schema test runs, then the decoy has no
  new commit and no tag, and the test passes.
- [ ] Given `GIT_DIR` set to a decoy, when `scripts/test-sensor-plan-frontier.sh` and
  `scripts/test-git-hooks.sh` run, then the decoy's refs and config are unchanged.
- [ ] Given `scripts/check-all.sh`, when it starts under a `GIT_DIR`, then its checks run as they
  do without one.

## Read first

- [ADR-0002](../../adr/0002-decisions-verified-by-tests.md) (the self-tests on fixture
  repositories); `.githooks/pre-push` and the commit `10cad02` that cleared the variables there;
  the convention *A test's fixture repository runs git without git's repository variables*.

## Blocked by

- None (can start immediately)

## Mode

with the human: the schema test is the style document's versioning check
([ADR-0007](../../adr/0007-harness.md)), and the self-tests are the template's; the fix may
belong upstream in the template too.
