# Ticket: Take up NUC v0.10.0, root option, script modes, and measures

## What to build

Forager takes up NUC v0.10.0:

- Each check of `check-docs.sh` is named by its rule key.
- Every script that can run on another repository takes its root only as `--root <dir>` and
  exits 2 on an argument it does not read. This covers Forager's own scripts under `scripts/`,
  through `scripts/lib/root.sh`.
- Every tracked script is mode 100755.
- `check-all.sh` names the seconds of each check.
- The pull request template carries the `Measures:` line and the four fields of the Doubts.
- The sensors of red commits, release notes, and work measures arrive.
- The design revision falls due after 50 changes; Forager edits its line from 20 to 50, as the
  maintainer decided on 2026-11-20.

Every caller of a script with a positional root moves to `--root`: hooks, workflows, skills,
documents, and the scheduled workflows. The `Template release` line reads v0.10.0.

## Acceptance criteria

- [ ] Given each script of Forager's own under `scripts/`, other than the self-tests and
      `check-all.sh`, when `scripts/check-shell.sh` runs, then the rules `shell/root`,
      `shell/bash-floor`, and `shell/mode` pass.
- [ ] Given a call of a script with a positional root anywhere in the tree, when the tree is
      searched, then none is left. Given such a call, when the script runs, then it exits 2.
- [ ] Given each ADR of Forager's that names a check of `check-docs.sh` by number, or cites
      clause 7 or 9 of NUC-checks-are-bash or clause 3 of NUC-design-is-revised, when the step
      is reviewed, then the ADR names the rule key or still holds against the wider clause.
      Otherwise a superseding ADR is accepted by the maintainer.
- [ ] Given 49 changes outside `docs/` since the last revision, when
      `scripts/sensor-revision-due.sh` runs, then it reports not yet due. At 50 it reports due.
- [ ] Given the change, when `scripts/check-all.sh` runs and the CI of the pull request runs,
      then both pass, and the `Template release` line of `README.md` reads v0.10.0.

## Read first

- Touches: `scripts/` (all of Forager's own scripts among them), `scripts/lib/root.sh` (new),
  `scripts/fixtures/check-shell.txt` (new), `.agents/skills/`, `.githooks/`,
  `.github/workflows/`, `.github/PULL_REQUEST_TEMPLATE.md`, `.editorconfig`, `.gitattributes`,
  `CONTRIBUTING.md`, `docs/adr/README.md`, the ADRs that name checks by number, `README.md`
- The release notes of v0.10.0: `gh release view v0.10.0 -R hivevm/nuc`, section *Taking it up*,
  with the map of check numbers to rule keys
- The template's diff `v0.9.1..v0.10.0`, cloned outside the tree
- The spec, *Implementation decisions*, on the revision line and the template findings

## Blocked by

- docs/plan/nuc-0.11.1/02-keys-and-tables-v0.9.1.md

## Mode

with the human: the release changes `NUC-` ADRs and can supersede ADRs of Forager's, and
OPS-parallel-sessions-capped-at-three keeps both with the maintainer. Stop and hand off with the
state on this ticket if the scripts of Forager's own do not all reach `--root` within the session.
Split the rest into a ticket of its own rather than land a half-moved caller.
