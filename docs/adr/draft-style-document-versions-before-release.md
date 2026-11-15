# ADR-draft-style-document-versions-before-release: The style document is versioned by a JSON Schema per format version, moved by the version rule until a release tag holds its schema and frozen from then, a newer minor read leniently by dropping what is unknown at its path, an older major migrated in steps, and a change of meaning a major

- **Status:** 🟡 proposed
- **Date:** 2026-11-15
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` (the style document, its readers in `document.rs`, `lenient.rs`, and `migration.rs`, its `schema` feature), `schema/` and `crates/style-ir/tests/schema.rs`, the `serde_ignored`, `serde_path_to_error`, and `schemars` entries of `Cargo.toml`, every read error of the style document that names a path, and every change to a field of the style IR
- **Supersedes:** [ADR-0014](0014-style-document-versions.md)

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

The current format version is 1.1, `FORMAT_VERSION` in `crates/style-ir/src/document.rs`, and its
schema is `schema/style-document-1.1.json`, the only file under `schema/`. Major 1 is the first, so
the chain of migration steps in `crates/style-ir/src/migration.rs` is empty. The release tags
`v0.1.0` to `v0.8.1` hold no schema file, so no format version is frozen yet, and the IR still
changes in place.

[ADR-0014](0014-style-document-versions.md) said that before the first release tag that holds a
schema file, "the version stays where it is". The tree no longer keeps that clause:

- 6881bce9 added the map's optional field `projection_auto` and moved the format version from 1.0
  to 1.1, a minor bump as the version rule sorts it, by the maintainer's choice of 2026-11-15. A
  reader of 1.0 then drops the field with a warning and reads the rest, where under 1.0 it would
  have refused the field as unknown to the current minor. The commit kept 1.0's file beside 1.1's.
- 52e68f9f removed `schema/style-document-1.0.json`. No release tag held it, and no check had read
  it since the bump.
- 52e68f9f also made a document read carry `FORMAT_VERSION` afterwards, whether it was read as the
  current or an older minor, leniently as a newer minor, or migrated from an older major. What is
  read is this build's IR, so it is written as this build's version. A document stamped 1.0 that
  carries `projection_auto` is written back as 1.1, which a reader of 1.0 reads leniently. A
  migrated document and its warning name the same version.
- The schema test passes any format version while no release tag holds a schema file
  (`verdict` in `crates/style-ir/tests/schema.rs`: "without a release, any version holds").

The maintainer decided this supersession in substance on 2026-11-15, as a supersession step of the
normalization of that day. It replaces the freeze clause with one that is true of the tree and
keeps every other clause of ADR-0014.

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
moved by the version rule before a release tag holds a schema and frozen from then, read a newer
minor by dropping what this build does not know at its path, migrate an older major one major at a
time on its value tree, and make any change of what a field means a major version.

- **Version rule.** A change may be a minor bump only when it adds an optional field, an enum
  value, or a CQL2 function, and changes the meaning of no field. Any other change to the shape
  must be a major bump. A change of what a field means, with or without a change of its shape,
  must be a major bump.
- **Conditions.** A new condition must be added only where a reader of an older minor reads what
  it does not know as never matching, such as a CQL2 function inside an existing expression. A
  new condition must never be a new top-level field.
- **Current version.** The current format version is 1.1. `FORMAT_VERSION` in
  `crates/style-ir/src/document.rs` must name it.
- **Schema files.** The `schema` feature of `style-ir` derives the current format version's
  schema. Its file, `schema/style-document-<version>.json`, must be checked in under `schema/`. A
  check derives the schema and compares it with the checked-in file of the current version. A
  difference fails the check.
- **Before a release.** While no release tag of Forager holds a schema file:
  - the change that alters the IR must regenerate the current version's file;
  - the maintainer may move the format version, and a move must follow the version rule;
  - `schema/` must hold the current version's file alone;
  - the change that moves the version must remove the file of the unreleased version it replaces.
- **Freeze.** Once a release tag of Forager holds schema files, those files must not change and
  must not be removed. From then, the check compares the derived schema with the schema of the
  last released format version. The current version must be the bump that change asks for:
  - the same version for no change;
  - the next minor for a change the version rule allows as a minor;
  - the next major for any other change.
- **The version written.** A document read must carry `FORMAT_VERSION` afterwards, whichever
  version it was read as, migrated, or read leniently from. Writing it must write
  `FORMAT_VERSION`.
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
whether the maintainer moves the version before a release, chosen change by change;
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
- **The version stays where it is until the first release, as ADR-0014's freeze clause stood** —
  one version for all unreleased work. But a field added under the same version is unknown to a
  reader built before it, and that reader refuses it as a field of its own minor. 6881bce9 moved
  to 1.1 for `projection_auto`, so that such a reader drops the field with a warning instead.
- **Every unreleased version's schema file kept under `schema/`** — the history at hand. No check
  reads a file but the current version's, so the others drift unseen and read as if published;
  git keeps the history.
- **A document written as the version it was read as** — no rewrite of the version. A document
  stamped 1.0 that carries a field of 1.1 would then be refused by a reader of 1.0, and a migrated
  document would name a version whose shape it no longer has.
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
- 6881bce9 (format version 1.1 for `projection_auto`) and 52e68f9f (the version written, 1.0's
  file removed); the release tags `v0.1.0` to `v0.8.1`, none of which holds `schema/`.

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
- Positive: the format version tells an older reader when a document carries more than it knows,
  even before the first release; a document read and written again names the version of its shape.
- Negative / trade-offs: until a release tag holds a schema, a moved version leaves no file of the
  version before it, so a reader built at that version is the only witness of its shape.
- Follow-ups: Forager's release process; publishing the schemas; a check that `schema/` holds the
  current version's file alone before a release; a span-aware location if
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
- an older major, and expect the migration through the chain, the document carrying
  `FORMAT_VERSION` afterwards (`an_older_major_is_migrated_step_by_step`);
- a document of 1.0 and one stamped 1.0 that carries `projection_auto`, and expect the current
  version 1.1 and the second written back as 1.1
  (`a_map_of_auto_reads_back_and_a_document_of_1_0_reads_as_before`);
- a newer minor whose filter, class expression, and label expression hold an unknown function
  (`a_newer_minors_unknown_function_never_matches_in_a_class_a_label_or_a_filter`), and expect
  each to never match.

The tests of `crates/style-ir/tests/schema.rs` cite this ADR with `Verifies:`:

- `the_checked_in_schema_is_the_one_the_ir_derives` fails when the current version's file is not
  the schema the IR derives;
- `the_bump_matches_the_change_since_the_last_release` fails when a released schema file changed
  or is gone, and when the current version is not the bump the change since the last release asks
  for;
- `without_a_release_only_the_file_must_be_fresh` and
  `a_clone_without_its_history_is_refused_and_one_without_a_release_passes` hold that any version
  passes while no release tag holds a schema file;
- `an_unchanged_schema_keeps_its_version_and_a_released_file_is_frozen` and the tests of the
  version rule on fixtures hold the freeze and each kind of bump.

`scripts/check-rust.sh` runs them with the `schema` feature. It fails when the browser bindings
depend on `schemars`.

No check holds that `schema/` holds the current version's file alone before a release. The review
of a change that moves `FORMAT_VERSION` holds it.

**Not mechanically decidable:** a change of meaning leaves the schema as it was, so no comparison
of schemas sees it. The review of a change to `crates/style-ir/src/model.rs` asks whether a field's
meaning moved.
