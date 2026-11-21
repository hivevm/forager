# Ticket: The self-tests' table readers refuse a row they do not read

## What to build

`scripts/test-check-docs.sh` and `scripts/test-check-devcontainer.sh` read their cases from tables:
the template's under `scripts/fixtures/`, and Forager's own rows beside them (`RECORD_OWN` in
`test-check-docs.sh`, the rows under a comment of Forager's own in `check-devcontainer.txt`). A
reader keeps the rows whose first field matches its pattern and skips every other line. When NUC
v0.10.0 changed the pattern from a check's number to its rule key, Forager's eight rows still
began with `17 |` and `9 |`. They were skipped without a word, and the rule
`docs/inactive-adr-refs` and the exemption of OPS-measurements-are-no-gate had no test until the
review of the take-up found it. Each reader must instead fail on a line that is neither blank, a
comment, nor a row it reads, naming the line.

## Acceptance criteria

- [ ] Given a row in `RECORD_OWN` whose first field is no rule key, when `test-check-docs.sh` runs,
      then it fails and names the row.
- [ ] Given a row of Forager's own in `check-devcontainer.txt` that the reader cannot read, when
      `test-check-devcontainer.sh` runs, then it fails and names the row.
- [ ] Given the tree, when both self-tests run, then every row is read and both pass; the count
      of their cases does not fall.

## Read first

- Touches: `scripts/test-check-docs.sh`, `scripts/test-check-devcontainer.sh`
- The header of each table under `scripts/fixtures/`, which states its format.
- `docs/CONVENTIONS.md`, "A check's cases are a table".
- Evidence: the review of commits 13114a68..f7668d35 in PR #222, point 1.

## Blocked by

- None (can start immediately)

## Mode

unattended:

- A test decides it: each criterion is a self-test case on a fixture row.
- Its scope is two self-tests, and a wrong result costs one revert.
- It touches no ADR and nothing OPS-parallel-sessions-capped-at-three keeps with the human.
- A wrong result shows as a failing self-test in `scripts/check-all.sh`; the revert undoes it.
- Stop: if a template row would fail the stricter reader, hand off with the row named here.
