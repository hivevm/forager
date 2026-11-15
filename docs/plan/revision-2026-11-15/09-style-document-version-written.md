# Ticket: A style document says the version it holds

## What to build

Format 1.1 left four places where the version a document carries and the version it is read as
disagree:

- A migration stamps each step's result `<major>.0` (`crates/style-ir/src/migration.rs:46-47`),
  so a migrated document is at "1.0". The warning says "migrated from format version 0.3 to 1.1"
  (`crates/style-ir/src/document.rs:225-226`). The test at `document.rs:1076-1086` asserts both
  side by side.
- A document stamped 1.0 that carries `projection_auto` reads with no warning, since the field is
  known. It writes back as "1.0" (`to_json`, `to_yaml`, `document.rs:129-137`), so a true 1.0
  reader then refuses it as `unknown-field` instead of reading it leniently as a newer minor.
- `README.md:79` shows a document at `version: "1.0"`. `crates/cli/tests/read.rs:32` asserts that
  `forager read` writes "1.1".
- `schema/style-document-1.0.json` is no longer compared by `crates/style-ir/tests/schema.rs`,
  and no release tag holds `schema/`. It is an unreleased draft that nothing checks.

With the maintainer: the version a migrated document and a written document carry, either the
version it was read as or `FORMAT_VERSION`, and whether the unreleased 1.0 schema file stays.
Then align the message, the stamp, the README, and the schema directory.

## Acceptance criteria

- [ ] Given a document of major 0, when it is read, then the version it carries and the version
  its `format-migrated` warning names are the same.
- [ ] Given a document stamped 1.0 whose map has `projection_auto`, when it is read and written
  again, then the version written is the one the maintainer chose, held by a test.
- [ ] Given `README.md`, when the change lands, then its sample document carries the version
  `forager read` writes.

## Read first

- Touches: `crates/style-ir/src/migration.rs`, `crates/style-ir/src/document.rs`,
  `crates/style-ir/tests/schema.rs`, `schema/`, `README.md`, `fuzz/fuzz_targets/style_document.rs`
- `docs/adr/0013-style-document-serde.md`, `docs/adr/0014-style-document-versions.md`; format
  1.1 is the maintainer's exception to ADR-0014's freeze clause, and ADR-0014 stays unchanged.

## Blocked by

None (can start immediately)

## Mode

with the human: what a style document says of its version is the maintainer's (ADR-0006), as the
exception of format 1.1 was.
