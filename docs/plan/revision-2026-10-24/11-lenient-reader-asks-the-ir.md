# Ticket: Give the lenient reader a module of its own that asks the IR where its conditions are

## What to build

`crates/style-ir/src/document.rs` has grown to a thousand lines, most of them the lenient reader
of a newer minor, and that reader knows the IR by copies:

- `condition` names the IR's conditions by field name (`layers/*/filter`,
  `classes/*/expression`), while `StyleDocument::bounded` and the `expressions` methods of
  `model.rs` list where expressions live a second and a third way; a condition added to the IR
  must be remembered in all three, and one forgotten in `condition` is dropped instead of kept as
  `false`, which matches every feature, the mistake the review of #42 found.
- `value_depth` counts depth on the value tree as `Expr::depth` counts it on the tree.
- The JSON Pointer is written by `pointer` (for `serde_ignored`) and `segments_pointer` (for
  `serde_path_to_error`), and unescaped again by `remove`.
- What failed is decided from serde's error text (`unknown variant`, `untagged enum`,
  `missing field`), held only by the tests that read each case.

The lenient reader moves to a module of its own; one list of the IR's conditions, next to the
model, serves `bounded` and the lenient reader; the depth is counted once; one type writes and
reads a JSON Pointer; and each match on serde's text is a named function with a test that pins
the text, so a serde update that rewords it fails there. Behaviour does not change.

## Acceptance criteria

- [ ] Given the tests of `style-ir`, when they run, then they pass unchanged.
- [ ] Given a test that adds no IR field, when it asks the IR's list of conditions, then it gets
  the layer's filter and the class's expression, and the lenient reader and `bounded` use that
  list.
- [ ] Given serde's error for an unknown variant, an untagged enum, and a missing field, when
  each named function sees it, then a test expects its verdict.
- [ ] Given the tree, when `replace('~', "~0")` is searched for, then it appears once.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0019](../../adr/0019-cql2-expressions.md); the proposed
  [ADR-0022](../../adr/0022-lenient-reader-path-to-error.md); the convention *A reader that
  skips what it cannot read fails closed* in `docs/CONVENTIONS.md`.
- `crates/style-ir/src/document.rs`, `crates/style-ir/src/model.rs`.

## Blocked by

- docs/plan/revision-2026-10-24/10-one-read-path-for-both-formats.md

## Mode

with the human: the style document's readers are its versioning, and the dropped-values bound is
a resource limit, both kept with the maintainer ([ADR-0007](../../adr/0007-harness.md)).
