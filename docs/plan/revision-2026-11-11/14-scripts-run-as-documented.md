# Ticket: Make every script the documents tell an agent to run executable

## What to build

[`AGENTS.md` §2](../../../AGENTS.md#2-start-here) tells every session to run
`scripts/sensor-binding-adrs.sh --sections`. Its mode in git is `100644`, so
`scripts/sensor-binding-adrs.sh` fails with "Permission denied", and a session has to know to call
it through `bash`. 20 scripts under `scripts/` outside `scripts/lib/` are `100644`, among them
`check-rust.sh`, `fuzz.sh`, `mutants.sh`, and `install-tools.sh`; `check-all.sh` calls them
through `bash`, so no check sees it. Most came with the template and the take-up of NUC v0.8.1
kept their modes. Set the mode of every script a document names as a command to `100755`, add a
check to `scripts/check-shell.sh` that such a script is executable, and raise the modes upstream.
The scripts under `scripts/lib/`, which are sourced, stay `100644`.

## Acceptance criteria

- [ ] Given a fresh clone, when `scripts/sensor-binding-adrs.sh --sections` runs, then it prints
  the binding ADRs.
- [ ] Given a script under `scripts/` with a shebang and mode `100644`, outside `scripts/lib/`,
  when `scripts/check-shell.sh` runs, then it fails and names the file, as its self-test shows.

## Read first

- Touches: `scripts/`, `scripts/check-shell.sh`, `scripts/test-check-shell.sh`
- [ADR-0004](../../adr/0004-template-releases.md) (what of the template a project may change),
  `CONTRIBUTING.md` on checks.

## Blocked by

- None (can start immediately)

## Mode

with the human: most of the files belong to the template, and whether the project changes them or
waits for a template release is the maintainer's choice under ADR-0004.
