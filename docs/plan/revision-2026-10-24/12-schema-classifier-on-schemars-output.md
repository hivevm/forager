# Ticket: Test the schema check's classifier on what schemars derives, not only on hand-written schemas

## What to build

The review of #43 found three ways the classifier of `crates/style-ir/tests/schema.rs` misread
what schemars really writes: it numbers each use of a generic type (`Bound`, `Bound2`, …), so an
added optional field read as major; a doc comment on an enum variant turns `enum` into `oneOf`
of `const`; and an untagged union's new alternative was taken for minor. Each was found by
editing `model.rs` by hand and fixed with tests on hand-written JSON, which hold only as long as
schemars keeps writing that shape. A schemars update that changes it again passes those tests.

The tests derive their pairs from fixture types instead: two versions of a small set of types
with `#[derive(Serialize, Deserialize, JsonSchema)]` in the test file, and `change` applied to
`schema_for!` of each. Hand-written JSON stays for the cases schemars cannot be made to write.

## Acceptance criteria

- [ ] Given fixture types `V1` and `V2` where `V2` adds an `Option<Bound<u32>>` beside an
  existing `Bound<f64>`, when their derived schemas are compared, then the change is minor.
- [ ] Given an enum without and with a doc comment on one variant, when compared, then there is
  no change; with one variant added, minor.
- [ ] Given an untagged enum with one alternative added, when compared, then major; a required
  field added, major.
- [ ] Each test cites ADR-0020.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md); `crates/style-ir/tests/schema.rs`;
  the review comment of #43.

## Blocked by

- None (can start immediately)

## Mode

with the human: the schema check is the style document's versioning, kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
