# Spec: Versions of the style document

## Problem

A developer who stores style documents instead of mapfiles cannot rely on them across builds of
Forager. A document written by a newer build loads only if it happens to add nothing the reader
does not know: an unknown field is dropped without a word, and an unknown value of a render mode
or a symbol type fails the whole load. Nothing states which change to the IR breaks old readers,
so a change that does goes unnoticed, and there is no way to read a document of an older major
version once one exists. There is no schema a tool or an editor could validate a document
against.

## Solution

Each format version of the style document is defined by a JSON Schema, checked in and derived
from the IR's types, and a check fails a change to the IR whose shape does not match the bump of
the version it makes. A reader takes a document of a newer minor version and drops what it does
not know, with a warning naming each field or value by its JSON Pointer path; a newer major
version is a load error; a document of an older major is migrated to the current one, step by
step, with a warning naming where it came from.

## Existing behaviour

- The readers compare only the major of `version`: the same major loads, a newer one is
  `format-version-unsupported`, and a version that is not a number is `format-version-invalid`.
  Pinned by the unit tests of the style document; they stay green, their codes unchanged.
- A document of the current version reads back equal from YAML and JSON (ADR-0014), and the
  CLI's `forager read` tests read its output back; both stay.
- Readers return a document or one diagnostic. They will return the document with its warnings;
  the callers are the CLI's tests and the `style_document` fuzz target, and both move in the
  first ticket. No host reads a style document yet.
- Format version `1.0` stays the current version; the IR keeps growing in place under it until
  Forager's first release (ADR-0020's freeze).

## Goals served

- **G-4** — met: newer minor read with diagnostics, newer major refused, older major migrated.
- **Q-3** — kept: the lenient reader and the migration are fuzzed with the readers.
- **Q-5** — kept: the schema derivation never enters the browser bundle.

## User stories

1. As a developer embedding Forager, I want a document from a newer build of the same major to
   load in my older build, so that I can upgrade the writers before the readers.
2. As a developer, I want each field or value my build drops to be named with its path, so that
   I see what my map lost.
3. As a developer, I want a document of a newer major to fail with both versions named, so that I
   know to upgrade rather than see a wrong map.
4. As a developer, I want a document of an older major to load into the current IR, so that
   documents I stored stay usable after a major change.
5. As a tool author, I want a JSON Schema per format version in the repository, so that an editor
   can validate and complete a style document.
6. As a maintainer, I want a check that fails when the IR's shape changes without the right
   version bump, so that compatibility is decided in the diff and not by memory.

## Implementation decisions

- The readers wrap the format's deserializer so that every ignored key is reported with its JSON
  Pointer path; the same wrapper serves YAML and JSON (ADR-0020).
- What is lenient depends on the document's minor: newer than the reader's, unknown fields and
  enum values are dropped with a warning; the same or older, they are load errors.
- An enum the IR can grow is read leniently field by field; an unknown expression operator or
  function keeps its class or filter, which never matches (ADR-0019).
- Migration runs on a generic value tree before typed reading, one step per major; the chain is
  a list of steps, and a test registers a fixture step from a test-only major so that the chain
  is exercised before a second real major exists.
- The schema is derived behind a `schema` feature of the IR crate; a small tool writes the schema
  file of the current version, and the check compares a fresh derivation with it. The schema of
  an expression is that of ADR-0019's tree.
- Version rule and freeze as ADR-0020 states them: additive is minor, anything else major; a
  released version's file never changes.
- The styling-core spec changes the IR's shape ticket by ticket. Once the schema file exists, each
  such change regenerates it in the same pull request, under the same version until the first
  release.

## Testing decisions

- The style document's unit tests are the seam for reading, leniency, and migration, as the
  existing version tests are; one test decides all three parts of G-4 and carries its marker.
- The `style_document` fuzz target reads with the lenient reader and the migration.
- The schema check runs in `scripts/check-rust.sh`, with a test of its own that feeds it a changed
  shape without a bump.

## Out of scope

- A host command or binding that reads a style document instead of a mapfile.
- Forager's release process and tags; publishing the schemas at a URL.
- Lines and columns in the location of an unknown field.
- A second real major version.

## ADR impact

- Binding: ADR-0014 (serde, the two crates), ADR-0012 (two new crates, `serde_ignored` and
  `schemars`), ADR-0009 (the IR crate stays free of workspace dependencies), ADR-0007 (the format
  and its versioning stay with the maintainer), ADR-0019 (unknown operators and functions).
- New: ADR-0020, proposed with this spec, to be accepted before its first ticket lands.
