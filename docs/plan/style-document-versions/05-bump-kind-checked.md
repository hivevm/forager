# Ticket: Fail a shape change whose version bump does not match it, and freeze released versions

## What to build

The schema check of ticket 04 learns ADR-0020's version rule: it compares the derived schema
with the file of the last released version and classifies the difference — only optional fields,
enum values, or CQL2 functions added is a minor change, anything else major — and fails when the
current version's bump does not match it. A schema file that a release tag of Forager contains is
immutable. Before the first release tag, the check only requires the current file to be fresh, as
ticket 04 does.

## Acceptance criteria

- [ ] Given a released `1.0` and an added optional field under `1.0`, when the check runs, then
  it fails asking for `1.1`; under `1.1`, it passes.
- [ ] Given a removed or renamed field under `1.1`, when the check runs, then it fails asking for
  a new major.
- [ ] Given a released schema file edited, when the check runs, then it fails.
- [ ] Given no release tag, when the check runs, then it behaves as ticket 04's check.
- [ ] Given each case, a test feeds the check a fixture pair and expects its verdict (citing
  ADR-0020).

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md); ticket 04's tool and check.

## Blocked by

- docs/plan/style-document-versions/04-schema-file-per-version.md

## Mode

with the human: the style document's versioning is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
