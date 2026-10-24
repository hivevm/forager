# Ticket: Fail the schema check in a clone without its history instead of passing it as unreleased

## What to build

The schema check reads the release tags to find the frozen schema files and the last released
version (`released` in `crates/style-ir/tests/schema.rs`). In a shallow clone, or one fetched
without tags, it finds none and passes every change as before the first release; only a comment
in the `rust` job of `.github/workflows/checks.yml` (`fetch-depth: 0`) keeps CI from that. The
check fails instead when the repository is shallow (`git rev-parse --is-shallow-repository`),
naming `git fetch --unshallow --tags`, so that the comment becomes a check.

## Acceptance criteria

- [ ] Given a shallow fixture clone, when the check's release reading runs on it, then it fails
  naming the command that fetches the history.
- [ ] Given a full clone without a release tag, when the check runs, then it behaves as today.
- [ ] The test cites ADR-0020.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md); `crates/style-ir/tests/schema.rs`;
  the convention *A test's fixture repository runs git without git's repository variables*.

## Blocked by

- docs/plan/revision-2026-10-24/13-fixture-git-without-repository-variables.md

## Mode

with the human: the schema check is the style document's versioning, kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
