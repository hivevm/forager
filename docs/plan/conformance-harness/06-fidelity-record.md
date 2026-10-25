# Ticket: Record the share of passing cases per release

## What to build

The harness writes the number of in-scope cases and of passing ones; a record in the tree holds
that share for each release tag, and a check fails when the current share is below the record of
the last release (Q-4). A release adds its line to the record.

## Acceptance criteria

- [ ] Given a record whose last release passed 40 of 50 cases, when the harness passes 39 of 50,
      then the check fails and names both shares.
- [ ] Given the same or a higher share, then the check passes.
- [ ] Given cases moved into scope so that the share falls although more cases pass, then the
      check fails, as Q-4 measures the share.
- [ ] Given no record yet, then the check passes and says so.
- [ ] The check has a self-test, and a test cites Q-4 with a `Verifies:` marker.

## Read first

- Q-4 and ADR-0002 (checks are bash scripts with a self-test); one of the existing checks with its
  self-test as the golden path; ADR-0005 (release tags).

## Blocked by

- docs/plan/conformance-harness/05-geometry-cases.md

## Mode

unattended: the self-test decides every criterion; one script, its self-test, and a record, undone
by one revert; nothing ADR-0007 keeps with the maintainer. Stop after two attempts that leave the
self-test red.
