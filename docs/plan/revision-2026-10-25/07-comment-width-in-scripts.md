# Ticket: Hold comment lines of scripts and hooks to 100 columns

## What to build

The review of #75 found the header of `scripts/fetch-msautotest.sh` with one comment line of more
than 200 columns (fixed in d541a0c). Check 15 of `scripts/check-docs.sh` holds Markdown prose to
100 columns and `rustfmt` holds Rust code, but nothing holds the comments of shell scripts, where
the checks state their rules. `scripts/check-shell.sh` also refuses comment lines past 100
columns in the files it lints, a line that cannot break (a URL) exempt as in check 15. Ten lines
exceed it today: one in `scripts/check-notice.sh`, the project's own, and nine in files the
template ships: `.githooks/pre-commit`, `scripts/check-docs.sh`,
`scripts/test-check-traceability.sh`, `scripts/test-git-hooks.sh`,
`scripts/test-sensor-action-refs.sh`. Since those files and
`check-shell.sh` are the template's ([ADR-0005](../../adr/0005-template-releases.md)), the
change is best made upstream and taken up with a template release; the maintainer decides
whether to wait for it or to carry it here first.

## Acceptance criteria

- [ ] Given a script with a comment line of 101 columns, when `scripts/check-shell.sh` runs, then
  it fails naming the file and line; `scripts/test-check-shell.sh` decides it with a fixture.
- [ ] Given a comment line that holds only a URL past 100 columns, when
  `scripts/check-shell.sh` runs, then it passes.
- [ ] Given the repository, when the check lands, then every existing offender is rewrapped.

## Read first

- `scripts/check-shell.sh`, `scripts/test-check-shell.sh`, check 15 of `scripts/check-docs.sh`;
  [ADR-0005](../../adr/0005-template-releases.md).

## Blocked by

- None (can start immediately)

## Mode

with the human: the check and most offenders are the template's, and where the change is made
first is the maintainer's call.
