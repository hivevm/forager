# Ticket: Forager's own rules stand in scripts of its own, not in the template's

## What to build

Forager carries rules of its own inside files the template ships: the GitHub CLI login in
`scripts/check-devcontainer.sh` (SEC-gh-login-stays-on-host), the rule `docs/inactive-adr-refs`
and the exemption of OPS-measurements-are-no-gate in `scripts/check-docs.sh`, and their rows in
the template's tables and self-tests. Each take-up of a release then merges those files instead
of replacing them: the take-up of v0.11.1 merged 38 of them, ported each rule twice, and lost two
of them from their tests until a review found it. Move each rule into a check of Forager's own
with its own self-test and table, so that the template's files carry no change of Forager's but
the ones a convention asks of every script, such as the `GIT_DIR` guard.

## Acceptance criteria

- [ ] Given the tree, when `scripts/check-devcontainer.sh` and `scripts/check-docs.sh` are compared
      with NUC v0.11.1, then they differ only by the `GIT_DIR` guard, and their self-tests and
      tables equal the release's.
- [ ] Given a binding of the host's `~/.config/gh`, a superseded ADR cited outside the record, and
      a script only a measuring workflow runs, when `scripts/check-all.sh` runs, then each fails or
      passes as today, through a check of Forager's own whose self-test cites its ADR.
- [ ] Given SEC-gh-login-stays-on-host and OPS-measurements-are-no-gate, whose *Enforcement* names
      the template's checks, when the change lands, then a superseding ADR the maintainer accepted
      names the new checks, or the maintainer decided the wording stays.

## Read first

- Touches: `scripts/check-devcontainer.sh`, `scripts/check-docs.sh`, their self-tests,
  `scripts/fixtures/`, new checks under `scripts/`, `scripts/check-all.sh`,
  `.github/workflows/checks.yml`, `CONTRIBUTING.md`, `docs/adr/SEC-gh-login-stays-on-host.md`,
  `docs/adr/OPS-measurements-are-no-gate.md`
- The rule `docs/check-lists` of `scripts/check-docs.sh`: how a check of the project joins the
  lists, and how an exemption could live outside it, if it can.
- Evidence: PR #222, its three merges by hand and the review's point 1 on `RECORD_OWN`.

## Blocked by

- docs/plan/revision-2026-11-20/01-table-readers-refuse-unread-rows.md

## Mode

with the human: the *Enforcement* of two accepted ADRs names the checks this moves, and
OPS-parallel-sessions-capped-at-three keeps every ADR with the maintainer. Whether the exemption
of `docs/check-lists` can leave `check-docs.sh` at all is a design question to settle first.
