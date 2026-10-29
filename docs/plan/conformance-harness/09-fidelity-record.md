# Ticket: Record the share of passing cases per release

## What to build

The harness writes the number of in-scope cases and of passing ones, and of the cases among
them that the list marks blank; the share leaves the blank cases out, and they are reported
apart from it, so that they do not inflate it. A record in the tree holds that share for each
release tag `v<semver>`, and a check, run by the Rust checks after the harness, fails when the
current share is below the record of the last release (Q-4). A release adds its line to the
record. No required check or repository setting changes.

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
  self-test as the golden path; the release tags `v<semver>` as the architecture overview names
  them for the schema files.

## Blocked by

- docs/plan/conformance-harness/08-mapserver-noise.md

## Mode

unattended: the self-test decides every criterion; one script, its self-test, and a record, undone
by one revert; nothing ADR-0007 keeps with the maintainer. It counts against the cap of three open
agent branches of [ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap
allows. Stop after two attempts that leave the self-test red.
