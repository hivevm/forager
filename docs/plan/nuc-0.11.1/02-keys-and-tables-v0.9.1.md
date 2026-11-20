# Ticket: Take up NUC v0.9.1, keyed findings and case tables

## What to build

Forager takes up NUC v0.9.1:

- Every finding of `check-docs.sh` and `check-devcontainer.sh` ends in a key of the form
  `[<area>/<rule>:<reason>]`.
- The fixture tables expect keys and subjects, not wording.
- Every script a person or an agent runs stops on a bash below 4.4 and names the version it
  needs.
- The plain rules sensor ends a sentence after a link whose text is only a code span.

Each row Forager added to one of the four tables takes the new form. Where Forager changed one of
the four checks, its own findings end in a key too. The `Template release` line reads v0.9.1.

## Acceptance criteria

- [ ] Given a row Forager added to `check-devcontainer.txt`, `check-docs-record.txt`,
      `check-traceability.txt`, or `plain-rules.txt`, when its self-test runs, then the row
      expects a key and its subjects and passes.
- [ ] Given a finding of a check Forager changed, when the check fails on a fixture, then the
      finding ends in a key.
- [ ] Given a bash below 4.4, when a script that sources `scripts/lib/bash.sh` runs, then it
      exits 2 and names the version it needs; `scripts/test-check-all.sh` holds it.
- [ ] Given the change, when `scripts/check-all.sh` runs and the CI of the pull request runs,
      then both pass, and the `Template release` line of `README.md` reads v0.9.1.

## Read first

- Touches: `scripts/lib/bash.sh` (new), `scripts/lib/selftest.sh`, `scripts/check-*.sh`, every
  `scripts/sensor-*.sh` of the template, `scripts/fixtures/`, the self-tests the release names,
  `.agents/skills/normalize/remap.sh`, `.agents/skills/work-tickets/claim.sh`, `CONTRIBUTING.md`
  (Workflow step 2, Project Layout), `docs/CONVENTIONS.md`, `README.md`
- The release notes of v0.9.1: `gh release view v0.9.1 -R hivevm/nuc`, section *Taking it up*
- The template's diff `v0.9.0..v0.9.1`, cloned outside the tree
- `docs/CONVENTIONS.md`, the entry "A check's cases are a table" as v0.9.1 ships it

## Blocked by

- docs/plan/nuc-0.11.1/01-migrate-record-v0.9.0.md

## Mode

with the human: the release replaces files that `NUC.blobs` holds, and it changes the checks of
the record. ADR-0006 keeps the ADRs with the maintainer. The step is otherwise mechanical, and
`scripts/check-all.sh` decides it. Stop and hand off where a merge would change what one of
Forager's checks decides.
