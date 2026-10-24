# ADR-0022: A newer minor's document that does not read is located with `serde_path_to_error` and read again with the failed value dropped, at most a hundred times

- **Status:** 🟢 accepted
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` (the lenient reader of the style document in `document.rs`), the `serde_path_to_error` entry of `Cargo.toml`, and every read error of the style document that names a path

## Context

[ADR-0020](0020-style-document-compatibility.md) decides that a document of a newer minor drops
an enum value or an expression the reader does not know, with a warning naming its path, and
names `serde_ignored` and `schemars` as its crates. `serde_ignored` sees only keys a struct
skips; an unknown enum value or an untagged expression that matches no alternative is an error
that ends the typed read, and serde's error carries no path.
[ADR-0012](0012-crate-supply-chain.md) leaves the choice of each crate to the ADR that first
needs it. The style document readers took a third crate, `serde_path_to_error` (0.1.20,
`MIT OR Apache-2.0`, by the author of `serde_ignored`), which wraps a deserializer and reports
the path at which the read failed. The maintainer chose it with the work of ADR-0020 and set the
bound of Q-9, "the values a style document of a newer minor drops (100)"; no ADR records it, so a
reader of ADR-0020 finds a dependency and a loop it does not explain.

## Decision

We will read a document of a newer minor that fails its typed read from its value tree with
`serde_path_to_error`, drop or neutralize the value at the path it reports, and read again, at
most `MAX_DROPPED_VALUES` (100) times before the document is refused with
`too-many-unknown-values`; every other read error of the style document names its path the same
way.

- **What is dropped.** An unknown enum value unsets the field that holds it; a layer's filter or
  a class's expression that does not read becomes `false`, held to the nesting limit; a required
  field cannot be dropped, and its unknown value is the load error `unknown-value` naming it.
- **Where the read is tried.** Only a newer minor's document is read again; the current or an
  older minor fails on the first error, with its path.

**Out of scope:** unknown keys inside untagged enums, which neither crate sees; lines and columns
of the place, which ADR-0020 leaves out.

## Alternatives considered

- **A hand-written path tracker** — no dependency, but a wrapper of every `Deserializer` and
  `Visitor` method, the code `serde_path_to_error` already is.
- **One pass with lenient types** — each growable enum given a catch-all variant and each
  expression read as a value, so that one read suffices; it changes the IR's types and its
  schema for a reader concern, and every new enum must remember it.
- **Read once and refuse** — no loop and no bound, but then an unknown value fails the load,
  which ADR-0020 rejected.

## Sources / Prior art

- `serde_path_to_error` <https://docs.rs/serde_path_to_error>, version and licence from
  `cargo metadata`, read 2026-10-24.
- Q-9 of `docs/SPECIFICATION.md`; ADR-0020.

## Consequences

- Positive: an unknown value is dropped at its place and named by its JSON Pointer in YAML and
  JSON alike, and every read error says where it happened.
- Negative / trade-offs: a third crate in the readers; up to a hundred re-reads of a document
  that is already in memory; deciding what failed rests on serde's error text (`unknown variant`,
  `untagged enum`, `missing field`), held by the tests that read each case.
- Follow-ups: reporting unknown keys inside an untagged expression or bound.

## Enforcement

The tests of `crates/style-ir/src/document.rs` read a newer minor with an unknown enum value, an
unknown operator, a condition that does not read, a required unknown value, and more than a
hundred unknown values, and expect each warning or error with its path; the test of the bound
cites Q-9, and the tests cite this ADR with `Verifies:` once it is accepted.
