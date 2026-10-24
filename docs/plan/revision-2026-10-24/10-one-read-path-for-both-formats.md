# Ticket: Read YAML and JSON through one path of the style document reader

## What to build

`StyleDocument::from_yaml_with` and `from_json_with` (`crates/style-ir/src/document.rs`) are the
same procedure written twice: peek at the version, take an older major to the migration, read
typed through `serde_path_to_error` and `serde_ignored`, and fall back to the lenient reader from
the value tree for a newer minor. They differ only in the deserializer, the value-tree parse, and
the error code (`yaml-read`, `json-read`). A fix to one has to be made in the other; the review
of #42 found both moved together only because tests ran each format.

One function holds the procedure and takes the format as its parameter (the two ways to
deserialize, and the code); the two public readers become one-line calls. Behaviour does not
change.

## Acceptance criteria

- [ ] Given the tests of `crates/style-ir/src/document.rs`, when they run, then they pass
  unchanged.
- [ ] Given `document.rs`, when it is read, then the sequence version peek → migration → typed
  read → lenient fallback appears once.
- [ ] Given the `style_document` fuzz target, when `scripts/check-rust.sh` lints it, then it
  builds without change.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0014](../../adr/0014-style-document-serde.md); `crates/style-ir/src/document.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the style document's readers are its versioning, kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
