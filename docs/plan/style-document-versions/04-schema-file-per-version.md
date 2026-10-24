# Ticket: Derive the JSON Schema of the current format version and check it in

## What to build

A `schema` feature of the IR crate derives a JSON Schema for the style document with `schemars`,
and a small tool writes it as the schema file of the current format version. The check in
`scripts/check-rust.sh` derives it afresh and fails when it differs from the checked-in file,
naming the command that regenerates it. The browser bindings never enable the feature.

## Acceptance criteria

- [ ] Given the tool, when it runs, then it writes the schema file of version `1.0`, and a
  document `forager read` prints validates against it.
- [ ] Given a change to an IR type without regenerating the file, when the check runs, then it
  fails naming the regeneration command.
- [ ] Given the bindings' dependency tree, when it is listed, then `schemars` is not in it, and
  the bundle stays under Q-5's 3 MB.
- [ ] Given `cargo deny check`, when it runs, then `schemars` passes ADR-0012.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0012](../../adr/0012-crate-supply-chain.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); `scripts/check-rust.sh`.

## Blocked by

- None (can start immediately); ADR-0020 is accepted before it lands.

## Mode

with the human: it adds a dependency and defines the format's schema, both kept with the
maintainer ([ADR-0007](../../adr/0007-harness.md)).
