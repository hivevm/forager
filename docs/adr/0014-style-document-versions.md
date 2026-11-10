# ADR-0014: The style document is versioned by a JSON Schema per format version, a newer minor read leniently by dropping what is unknown at its path, an older major migrated in steps, and a change of meaning a major

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` (the style document, its readers in `document.rs`, `lenient.rs`, and `migration.rs`, its `schema` feature), `schema/` and `crates/style-ir/tests/schema.rs`, the `serde_ignored`, `serde_path_to_error`, and `schemars` entries of `Cargo.toml`, every read error of the style document that names a path, and every change to a field of the style IR

## Context

G-4 asks that a style document of a newer minor format version loads with its unknown fields
ignored and each named in a diagnostic with its location, that a newer major version is a load
error, and that an older major version is migrated to the current one. Q-9 bounds the values a
style document of a newer minor drops (100) and the nesting of an expression (24 levels). Q-4 asks
that the meaning MapServer gives a map survives. The style document is read and written through
serde ([ADR-0013](0013-style-document-serde.md)), and the harness keeps the style document format
and its versioning with the maintainer ([ADR-0006](0006-harness.md)).

serde alone ignores an unknown field without a word, fails the whole read on an unknown enum value
or on an untagged expression that matches no alternative, and names no path in its error.
`serde_ignored` (0.1.14, `MIT OR Apache-2.0`) wraps any serde `Deserializer` and reports each key a
struct skips with its path; `serde-saphyr` 2.0.0 exposes its deserializer through
`with_deserializer_from_str`, and `serde_json` through `Deserializer::from_str`, so one wrapper
serves both formats. `serde_path_to_error` (0.1.20, `MIT OR Apache-2.0`, by the same author) wraps
a deserializer and reports the path at which a typed read failed. `schemars` (1.2.3, `MIT`)
derives a JSON Schema from serde types and honours their serde attributes.

The current format version is 1.0, and its schema is `schema/style-document-1.0.json`. Major 1 is
the first, so the chain of migration steps in `crates/style-ir/src/migration.rs` is empty. The
release tags `v0.1.0` to `v0.6.0` hold no schema file, so no format version is frozen yet, and the
IR still changes in place.

A rule by the shape of the document alone misses two kinds of change, which the design revision
of 2026-10-31 found in the work since the previous revision, while the format was still unreleased:

- **A change of meaning without a change of shape.** `Style.line_offset` turned from "positive to
  the left" to "positive to the right" of a line; a bound label `size`, `priority`, `offset`, or
  `shadow_size` came to be rounded to an integer; what a `Bound::Binding` reads came to depend on
  the property it binds. Each left the schema as it was. After the first release, a document
  written by one version would draw differently in the next, and a comparison of schemas would
  pass without a bump.
- **A new condition field.** `labels[].expression` was added as an optional field, a minor bump
  by shape. A reader of an older minor drops an unknown field with a warning, and would draw every
  label the condition would have left out. A condition that does not read, by contrast, is kept
  and never matches, as the convention that a reader skipping what it cannot read fails closed
  holds ([`docs/CONVENTIONS.md`](../CONVENTIONS.md)). A dropped condition widens what a map draws.

## Decision

We will version the style document by a JSON Schema per format version derived with `schemars`,
read a newer minor by dropping what this build does not know at its path, migrate an older major
one major at a time on its value tree, and make any change of what a field means a major version.

- **Version rule.** A change may be a minor bump only when it adds an optional field, an enum
  value, or a CQL2 function, and changes the meaning of no field. Any other change to the shape
  must be a major bump. A change of what a field means, with or without a change of its shape,
  must be a major bump.
- **Conditions.** A new condition must be added only where a reader of an older minor reads what
  it does not know as never matching: as a CQL2 function inside an existing expression. A new
  condition must not be a new field of its own.
- **Schema files.** The `schema` feature of `style-ir` derives each format version's schema, and
  the file is checked in under `schema/`. A check derives the schema and compares it with the
  checked-in file of the current version. A difference without a bump fails the check. The kind
  of bump must match the difference.
- **Freeze.** A format version's schema file is immutable once a release tag of Forager contains
  it. Before the first such tag, the current version's file is regenerated in the same change as
  the IR, and the version stays where it is.
- **Feature.** The tool and the tests that write or compare schema files may enable the `schema`
  feature. The browser bindings must not enable it (Q-5).
- **Unknown fields.** A reader drops an unknown field of a newer minor with a warning that names
  the field and its location as a JSON Pointer path, in YAML and JSON alike. `serde_ignored`
  finds them, and the IR's own reader of an expression's or a binding's keys reports the keys
  inside them, which an untagged enum would skip unseen.
- **Unknown values.** When a newer minor's document fails its typed read, the reader reads it from
  its value tree with `serde_path_to_error`, drops or neutralizes the value at the path it reports,
  and reads again:
  - An unknown enum value unsets the field that holds it, as if it were not set, with a warning
    naming its path.
  - A layer's filter, a class's expression, or a label's expression that does not read, an unknown
    operator or function among them, becomes `false`, which never matches. The element it sits on
    is kept, with a warning naming its path. The value it replaces is still held to the nesting
    limit.
  - A required field cannot be dropped. Its unknown value is the load error `unknown-value`,
    naming its path.
  - The reader reads again at most `MAX_DROPPED_VALUES` (100) times. Past that, the document is
    refused with `too-many-unknown-values` (Q-9).
- **Current and older minors.** A document of the current or an older minor is read once. An
  unknown field or value in it is a load error naming its path, since no version its writer knew
  has it. Every other read error of the style document names its path the same way.
- **A newer major** is a load error naming both versions.
- **An older major** is migrated by a chain of steps, each from one major to the next, on the
  document's value tree, before the typed read. Each step is a function with its own tests. A
  migrated document carries a warning naming the version it came from.

**Out of scope:** Forager's release process and its tags, which decide when the freeze applies;
publishing the schema at a URL a document's `$schema` can name; validating documents against the
schema at load, which the typed reader already does; lines and columns of a place; how a
migration step rewrites a field whose meaning moved; the changes of meaning made before the first
release, which the freeze leaves free.

## Alternatives considered

- **Path and line/column for unknown fields** — what a cartographer would like best, but
  `serde_json` keeps no span for an ignored key; it needs a span-aware reader for each format.
- **No schema files, only the rule in the conventions** — nothing then holds the rule but review;
  a check over a derived schema holds it mechanically.
- **The schema at run time only, `forager schema`** — no stable file for a version, so no freeze
  and no diff.
- **A handwritten schema** — no dependency, but two descriptions of one shape that drift.
- **Unknown enum values as a load error** — strict, but then every new render mode or symbol type
  is a major bump, and a newer document fails in an older viewer for one field.
- **An unknown operator as a load error** — expressions are semantic and guessing is dangerous;
  never matching is the same treatment [ADR-0015](0015-expressions-as-cql2.md) gives to what the
  importer cannot carry.
- **Read once and refuse** — no loop and no bound, but then an unknown value fails the load,
  which the lenient reading rejects.
- **A hand-written path tracker** — no dependency, but a wrapper of every `Deserializer` and
  `Visitor` method, the code `serde_path_to_error` already is.
- **One pass with lenient types** — each growable enum given a catch-all variant and each
  expression read as a value, so that one read suffices; it changes the IR's types and its schema
  for a reader concern, and every new enum must remember it.
- **Migration on typed structs per major** — type-safe, but every old major's types stay in the
  crate for ever; a value tree keeps one set of types.
- **Freeze now** — every IR change of the styling core would bump a minor before anything is
  released, with no reader of the old versions to serve.
- **Shape only, as the version rule first stood** — the check decides it alone, but a document of
  an older version silently draws differently, against Q-4, and a dropped condition draws what it
  should not.
- **Every new optional field a major bump** — safe, but it gives up G-4's lenient reading of a
  newer minor for every addition, not only for those that widen a map.
- **A new field per meaning (`line_offset_right` beside `line_offset`)** — no bump, but the IR
  carries every meaning it ever had.
- **A minor bump where a change of meaning only widens the values accepted** — a document written
  by the older version would still draw the same, but the line between widening and changing is a
  judgement each change would argue anew; every change of meaning is a major.
- **A new condition field as a major bump, or a list of fields an older reader must not drop** —
  the first gives up the minor for every condition; the second adds a mechanism to the document
  that a condition inside an existing expression does not need.

## Sources / Prior art

- `serde_ignored` <https://docs.rs/serde_ignored>, `serde_path_to_error`
  <https://docs.rs/serde_path_to_error>, `schemars` <https://docs.rs/schemars>, licences and
  versions from crates.io and `cargo metadata`, read 2026-10-24; `serde-saphyr` 2.0.0
  `src/de/with_deserializer.rs`.
- JSON Pointer, RFC 6901.
- [Semantic Versioning 2.0.0](https://semver.org/): a minor version is additive, and a
  backwards-incompatible change of the public API, whatever its form, is a major version.
- JSON Schema and Protocol Buffers' guidance on unknown fields: an unknown field may be ignored
  only where ignoring it is safe; Protocol Buffers warns against reusing a field for a new meaning.
- The style document's own history between `4c42c42` and `6745da4` (the line offset's sign, the
  rounded label bindings, `labels[].expression`).

## Consequences

- Positive: G-4 is met with one reader path for both formats; an unknown value is dropped at its
  place and named by its JSON Pointer in YAML and JSON alike, and every read error says where it
  happened.
- Positive: a change to the IR states its compatibility in a diff a reviewer sees and a check
  decides; old documents stay readable without old types; a document of a released version keeps
  its meaning in every later minor; a newer minor's condition never widens an older reader's map.
- Negative / trade-offs: three crates in the readers, `schemars` only behind a feature; unknown
  fields are located by path, not by line; a migration step on a value tree is checked by its
  tests, not by the compiler.
- Negative / trade-offs: up to a hundred re-reads of a document already in memory; deciding what
  failed rests on serde's error text (`unknown variant`, `untagged enum`, `missing field`), held by
  the tests that read each case.
- Negative / trade-offs: a change of meaning cannot be told from a shape and rests on review; a
  condition that could have been a small optional field waits for a major or takes the form of a
  function in an expression.
- Follow-ups: Forager's release process; publishing the schemas; a span-aware location if
  cartographers need lines; how review or a check names a change of meaning; the migration step
  of the first change of meaning after the freeze.

## Enforcement

The tests of `crates/style-ir/src/document.rs` cite this ADR with `Verifies:`. They read:

- a newer minor with an unknown field, an unknown key inside an expression or a binding, an
  unknown enum value, an unknown operator, a condition
  that does not read, and a required unknown value, and expect each warning or error with its
  path;
- more than a hundred unknown values, and expect `too-many-unknown-values`; that test also cites
  Q-9;
- a newer major, and expect the load error;
- an older major, and expect the migration through the chain;
- a newer minor whose filter, class expression, and label expression hold an unknown function
  (`a_newer_minors_unknown_function_never_matches_in_a_class_a_label_or_a_filter`), and expect
  each to never match.

The tests of `crates/style-ir/tests/schema.rs` cite this ADR with `Verifies:`. They fail when the
checked-in schema is not the one the IR derives, and when the bump since the last released format
version does not match the change. `scripts/check-rust.sh` runs them with the `schema` feature and
fails when the browser bindings depend on `schemars`.

**Not mechanically decidable:** a change of meaning leaves the schema as it was, so no comparison
of schemas sees it. The review of a change to `crates/style-ir/src/model.rs` asks whether a field's
meaning moved.
