# Ticket: Read a newer minor's unknown fields with a warning naming each path

## What to build

The tracer bullet: `StyleDocument`'s YAML and JSON readers return the document together with
its warnings, and a document of a newer minor of the current major that carries a field the
reader does not know loads, with a warning per field that names it and its JSON Pointer path. In
a document of the current or an older minor, an unknown field is a load error with the same
path. The CLI's tests and the `style_document` fuzz target move to the new return type.

## Acceptance criteria

- [ ] Given `{"version":"1.9","map":{"layers":[{"name":"a","glow":1}]}}`, when it is read as
  JSON, then it loads and one warning names `glow` at `/map/layers/0/glow` (citing ADR-0020).
- [ ] Given the same document as YAML, when it is read, then the warning is the same.
- [ ] Given the same field in a `1.0` document, when it is read, then it is a load error naming
  the path.
- [ ] Given a `2.0` document and a version that is not a number, when they are read, then the
  existing codes `format-version-unsupported` and `format-version-invalid` stay.
- [ ] Given the bundle, when `scripts/check-rust.sh` measures it, then it stays under Q-5's 3 MB.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0014](../../adr/0014-style-document-serde.md),
  [ADR-0012](../../adr/0012-crate-supply-chain.md); G-4 of `docs/SPECIFICATION.md`.
- The style document and its unit tests; `serde_ignored`;
  `serde_saphyr::with_deserializer_from_str`.

## Blocked by

- None (can start immediately); ADR-0020 is accepted before it lands.

## Mode

with the human: the style document's versioning is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
