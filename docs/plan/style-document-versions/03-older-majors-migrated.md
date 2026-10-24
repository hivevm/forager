# Ticket: Migrate a document of an older major step by step, and verify G-4

## What to build

A document whose major is older than the current one is read into a generic value tree, passed
through the chain of migration steps from its major to the current one, and then read as the
current version, with a warning naming the version it came from. The chain is a list of steps,
each from one major to the next; a test registers a fixture step from a test-only major so that
the chain runs before a second real major exists. One test decides all of G-4: a newer minor with
an unknown field, a newer major, and an older major migrated.

## Acceptance criteria

- [ ] Given a fixture step from major 0 that renames a field, when a `0.3` document is read with
  it, then it loads as the current version with the field renamed and a warning naming `0.3`.
- [ ] Given an older major with no step to the next, when it is read, then it is a load error
  naming the version.
- [ ] Given the three cases of G-4, when one test reads them, then each behaves as G-4 says, and
  the test cites G-4.
- [ ] Given the `style_document` fuzz target, when it runs, then it reads through the migration
  path too.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md); G-4 of `docs/SPECIFICATION.md`; the
  convention *A `Verifies:` marker claims every shall of its criterion* in `docs/CONVENTIONS.md`.
- Ticket 01's reader; `fuzz/fuzz_targets/style_document.rs`.

## Blocked by

- docs/plan/style-document-versions/01-unknown-fields-named-by-path.md

## Mode

with the human: the style document's versioning is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
