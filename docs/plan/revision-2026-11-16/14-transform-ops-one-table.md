# Ticket: The transform tree's unary operators share one variant and one table of names

## What to build

Five nodes of the transform tree have one shape, a shape and a number: `generalize`, `densify`,
`simplify`, `simplifypt`, and `buffer` (ADR-0048). Each is a variant of its own of `TransformShape`
(`crates/style-ir/src/transform.rs`). Their names are written six times in `transform.rs`
(the variants, the size, the serializer, the reader), twice in the mapfile frontend
(`crates/mapfile/src/expression/transform.rs`), and again as budget labels in
`crates/symbolize/src/layer_transform.rs`. ADR-0043 and then ADR-0048 each added operators by
writing every list again.

The five become one variant of an operator and its two arguments. One table of the operator's
names serves the serializer, the reader, and the frontend. The JSON the style document writes and
reads does not change: `schema/style-document-1.1.json` and every fixture stay as they are. This
is a `refactor` of the IR's Rust types, not of the style document (ADR-0006, ADR-0013).

## Acceptance criteria

- [ ] Given the tree, when `grep -rn '"simplifypt"' crates/*/src` runs, then it finds the one
  table.
- [ ] Given the schema test of the style document, the frontend's tests of `GEOMTRANSFORM`, and
  `cargo test --workspace`, when they run, then all pass unchanged and the schema file is not in
  the diff.
- [ ] Given ticket 02 of this revision, when its check runs, then it reads the operators from the
  table, and the ticket's change updates the check in the same commit.

## Read first

- Touches: `crates/style-ir/src/transform.rs`, `crates/mapfile/src/expression/transform.rs`,
  `crates/symbolize/src/layer_transform.rs`, `crates/engine/src/classes.rs`,
  `fuzz/fuzz_targets/render.rs`
- ADR-0048, ADR-0013, ADR-0014; `schema/style-document-1.1.json`.

## Blocked by

- docs/plan/revision-2026-11-16/02-check-transform-variants-fuzzed.md (it reads the variants)
- docs/plan/revision-2026-11-16/12-geometry-owns-the-geos-boundary.md (same module)

## Mode

with the human: the transform tree is the style IR's, which ADR-0006 keeps with the maintainer
even where its JSON does not change.
