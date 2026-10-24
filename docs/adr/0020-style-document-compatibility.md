# ADR-0020: The style document is versioned by a JSON Schema per format version, read leniently across minors, and migrated across majors in steps

- **Status:** 🟡 proposed
- **Date:** 2026-10-24
- **Deciders:** Markus Brigl
- **Applies to:** `crates/style-ir` (the style document, its readers, its `schema` feature), the schema files of the style document and the check that compares them, and every change to the shape of the style IR

## Context

G-4 asks that a style document of a newer minor format version loads with its unknown fields
ignored and each named in a diagnostic with its location, that a newer major version is a load
error, and that an older major version is migrated to the current one. ADR-0014 decided serde
for reading and writing and left out the JSON Schema of each version and the migration of older
majors. The harness keeps the style document format and its versioning with the maintainer
(ADR-0007).

Today a reader compares only the major of `version`; serde ignores an unknown field without a
word, an unknown enum value fails the whole load, and no rule says when a change to the IR moves
the version. There is only major 1, so there is nothing yet to migrate. The IR grows in place
through the styling-core spec, and Forager has no release yet: no document of another build has
been promised to read.

`serde_ignored` (0.1.14, `MIT OR Apache-2.0`) wraps any serde `Deserializer` and reports each
ignored key with its path; `serde-saphyr` 2.0.0 exposes its deserializer through
`with_deserializer_from_str`, and `serde_json` through `Deserializer::from_str`, so one wrapper
serves both formats. `schemars` (1.2.3, `MIT`) derives a JSON Schema from serde types and honours
their serde attributes.

## Decision

We will define each format version of the style document by a JSON Schema derived from the IR's
types with `schemars` and checked in, read a document of a newer minor by dropping what the
current version does not know with a warning naming its path, and read a document of an older
major by migrating it one major at a time on a generic value tree before typed deserialization.

- **Version rule.** A change that only adds an optional field, an enum value, or a CQL2 function
  is a minor bump; any other change to the shape is a major bump. A check derives the schema and
  compares it with the checked-in file of the current version: a difference without a bump
  fails, and the kind of bump must match the difference.
- **Freeze.** A format version's schema file is immutable once a release tag of Forager
  contains it; before the first release, the current version's file is regenerated in the same
  change as the IR, and the version stays where it is.
- **Unknown fields** of a newer minor are dropped with a warning that names the field and its
  location as a JSON Pointer path, in YAML and JSON alike.
- **Unknown enum values** of a newer minor drop the field that holds them, as if it were not
  set, with the same warning; an unknown operator or function in an expression keeps its class
  or filter, which never matches (as ADR-0019 does for constructs outside its subset).
- **A newer major** is a load error naming both versions. In a document of the current or an
  older minor, an unknown field or value is a load error, since no version that writer knew has
  it.
- **An older major** is migrated by a chain of steps, each from one major to the next, on the
  document's value tree; each step is a function with its own tests, and a migrated document
  carries a warning naming the version it came from.
- **The `schema` feature** of `style-ir` derives the schemas; it is enabled by the tool and the
  tests that write or compare schema files, never by the browser bindings (Q-5).

**Out of scope:** Forager's release process and its tags, which decide when the freeze applies;
publishing the schema at a URL a document's `$schema` can name; validating documents against the
schema at load, which the typed reader already does.

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
  never matching is the same treatment ADR-0019 gives to what the importer cannot carry.
- **Migration on typed structs per major** — type-safe, but every old major's types stay in the
  crate for ever; a value tree keeps one set of types.
- **Freeze now** — every IR ticket of the styling core would bump a minor before anything is
  released, with no reader of the old versions to serve.

## Sources / Prior art

- `serde_ignored` <https://docs.rs/serde_ignored>, `schemars` <https://docs.rs/schemars>, licences
  and versions from crates.io, read 2026-10-24; `serde-saphyr` 2.0.0
  `src/de/with_deserializer.rs`.
- JSON Pointer, RFC 6901.
- Semantic Versioning 2.0.0, <https://semver.org>, for the minor-is-additive rule.

## Consequences

- Positive: G-4 is met with one reader path for both formats; a change to the IR states its
  compatibility in a diff a reviewer sees and a check decides; old documents stay readable
  without old types.
- Negative / trade-offs: two dependencies, one of them only behind a feature; unknown fields are
  located by path, not by line; a migration step on a value tree is checked by its tests, not by
  the compiler.
- Follow-ups: Forager's release process; publishing the schemas; a span-aware location if
  cartographers need lines.

## Enforcement

Tests of `style-ir` read a document of a newer minor with an unknown field and an unknown enum
value and expect each warning with its path, read a newer major and expect the load error, and
migrate a fixture of an older major through the chain; the schema check fails on a shape change
without the matching bump; all cite this ADR with `Verifies:`.
