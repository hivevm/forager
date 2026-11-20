# Ticket: Take up NUC v0.11.0 and v0.11.1, linear integration

## What to build

Forager takes up NUC v0.11.0 and its patch v0.11.1 in one step. From v0.11.0:

- A reviewed branch is rebased onto its `feat/` branch, and the `feat/` branch moves forward by a
  fast-forward.
- The hooks refuse a merge on a `feat/` branch.
- `scripts/ready-for-main.sh` takes up `origin/main` by a rebase.
- The review binding sensor reads the pull request's description with `--hand-off <file>`, not
  merge messages.

From v0.11.1:

- The plain rules sensor reads an ADR identifier as one token.
- The skills read as lists.
- The self-tests' fixture guard holds.

Forager's skills, hooks, workflows, and `CONTRIBUTING.md` follow both. The `Template release`
line reads v0.11.1, and the spec is ready for its design revision.

## Acceptance criteria

- [ ] Given a `feat/` branch, when a merge is attempted on it, then the `pre-merge-commit` or
      `pre-commit` hook refuses it; `scripts/test-git-hooks.sh` holds it.
- [ ] Given a branch that carries a merge commit `origin/main` lacks, when
      `scripts/ready-for-main.sh` runs, then it refuses and names each such merge.
- [ ] Given a pull request description with a review at the head, when
      `scripts/sensor-review-binding.sh --hand-off <file>` runs, then the review binds every
      commit. `.github/workflows/review-binding.yml` passes it the description.
- [ ] Given an ADR identifier in a rule line, when `scripts/sensor-plain-rules.sh --require`
      runs, then it reports no avoided word inside the identifier.
- [ ] Given the change, when `scripts/check-all.sh` runs and the CI of the pull request runs,
      then both pass, and the `Template release` line of `README.md` reads v0.11.1.

## Read first

- Touches: `.githooks/pre-commit`, `.githooks/pre-merge-commit`, `.githooks/pre-push`,
  `scripts/ready-for-main.sh`, `scripts/sensor-review-binding.sh`,
  `scripts/sensor-release-notes.sh`, `scripts/sensor-plain-rules.sh`, `scripts/lib/selftest.sh`,
  `scripts/check-shell.sh`, `scripts/lib/bash.sh`, their self-tests and fixtures,
  `.agents/skills/`, `.github/PULL_REQUEST_TEMPLATE.md`, `.github/workflows/review-binding.yml`,
  `CONTRIBUTING.md` (Coding Agents, Git hooks, Integration branches, Cutting a release,
  Repository settings), `README.md`
- The release notes of v0.11.0 and v0.11.1: `gh release view v0.11.0 -R hivevm/nuc` and
  `gh release view v0.11.1 -R hivevm/nuc`, sections *Taking it up*
- The template's diff `v0.10.0..v0.11.1`, cloned outside the tree

## Blocked by

- docs/plan/nuc-0.11.1/03-root-mode-measures-v0.10.0.md

## Mode

with the human: the release replaces `NUC-` ADRs, clause 5 of NUC-main-only-via-pr among them, and
it changes how every later branch integrates. OPS-parallel-sessions-capped-at-three keeps the ADRs
with the maintainer.
