# Ticket: A check holds the comments of shell scripts and workflows to 100 columns

## What to build

`scripts/check-comment-width.sh` holds a Rust comment to 100 characters. The comments of the shell
scripts and of the workflows have no such check, and the migration of the record to identifiers
pushed some 35 shell comment lines and 8 lines of `checks.yml` past 100 columns, which the review
of PR #222 found by hand (point B4). NUC v0.11.1 holds the comments of the scripts it ships to 100
columns, and says so. Extend the check, or add one beside it, to the comment lines of
`scripts/**/*.sh`, `.githooks/*`, `.agents/skills/*/*.sh`, and `.github/workflows/*.yml`, and
rewrap the lines over the limit in a commit of their own that changes only line breaks.

## Acceptance criteria

- [ ] Given a `#` comment line of 101 characters in a script under `scripts/`, when the check runs,
      then it fails naming the file, the line, and the width.
- [ ] Given a comment of 100 characters, some multi-byte, or a long string or here-document line,
      when the check runs, then it passes.
- [ ] Given the tree after the rewrap, when `scripts/check-all.sh` runs, then the check passes.

## Read first

- Touches: `scripts/check-comment-width.sh`, `scripts/test-check-comment-width.sh`,
  `scripts/check-all.sh`, `.github/workflows/checks.yml`, the scripts and workflows it rewraps
- The golden path: `scripts/check-comment-width.sh` and its self-test, ticket
  revision-2026-11-18/13 (merge c24ed6a6).
- The rule `docs/check-lists` of `scripts/check-docs.sh`, which holds the run lines and the jobs.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides it: the self-test's cases are the criteria.
- Its scope is a check and line breaks, and a wrong result costs one revert.
- It touches no ADR and nothing OPS-parallel-sessions-capped-at-three keeps with the human.
- A wrong result shows as a failing check or self-test; the revert undoes it.
- Stop: if a line cannot be wrapped without changing a word, such as a `Verifies:` marker or a
  MapServer notice, hand off with the lines named here.

## Stopped (2026-11-21)

The session stopped at the *Mode*'s stop before building anything: one line cannot be wrapped
without changing a word.

- `scripts/test-check-shell.sh:21`, its `# Inputs:` line, is 224 characters. `scripts/check-all.sh`
  reads only the first line that starts with `# Inputs:` (`grep -m1 '^# Inputs:'`, line 94), so a
  path moved to a second line drops out of `--changed`. The other `# Inputs:` lines fit (the widest
  of the rest is 100, `scripts/test-sensor-tests-kept.sh`).
- The ways on are the maintainer's to choose: the check passes a `# Inputs:` line as a marker it
  does not wrap; `check-all.sh` reads continued `# Inputs:` lines; or the line names a directory
  for several paths, which changes words.

Where it stands, on the tree at b8e83891, counting characters of the lines whose first characters
after the indentation are `#`:

- 57 such lines over 100 in 30 files: 13 in the workflows (`checks.yml` 9, `noise.yml` 2,
  `fuzz.yml` 1, `mutants.yml` 1), 44 in `scripts/` (`check-docs.sh` 5, `check-rust.sh` 5, three
  `Verifies:` lines, the `# Inputs:` line above, and `test-check-docs.sh:14` and `:311`). None in
  `.githooks/` or `.agents/skills/*/*.sh`.
- The three `Verifies:` lines are no stop: a break before `Verifies:` keeps each marker whole on
  its own line, and `scripts/check-traceability.sh` reads a marker anywhere in the file
  (`scripts/test-build-mapserver.sh:3`, `scripts/test-sensor-diagnostics-untested.sh:3`,
  `scripts/test-sensor-duplication.sh:3`).
- No MapServer notice and no here-document line is among the 57.
