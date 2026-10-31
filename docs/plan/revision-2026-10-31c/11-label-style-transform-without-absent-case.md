# Ticket: Settle what a style without a transform means on a label and on a class

## What to build

`Style.geom_transform` (`crates/style-ir/src/model.rs`) is an `Option<GeomTransform>` whose absent
case means two things: on a class's style, the style draws at the feature's own geometry; on a
label's style, the style is not drawn, so the frontend writes `labelpoint` for a label's `STYLE`
without a `GEOMTRANSFORM` (the doc comments of `geom_transform` and `GeomTransform::LabelPoint`).
The maintainer decided the label's meaning in #156, so that a transform the frontend drops is not
taken for the default; the same field, read without knowing whose style it is, means the opposite,
and an engine, a reader, or a hand-written document that forgets which can draw what was meant to
be refused. Two ways out, the maintainer's to choose:

- **A label style's transform of its own** — a type with only the label transforms
  (`labelpoly`, `labelpoint`, `labelcenter`) and no absent case, on a label style of its own or
  on a field a label's style carries instead, so a label style without one cannot be written.
  It changes the style document's shape, under ADR-0020's freeze while the format is unreleased.
- **Keep the rule, and record it** — the field stays, and the two meanings of its absent case
  become a decision of the record through the skill `propose-adr`, so a reader of the schema finds
  it decided and does not take it for a mistake.

## Acceptance criteria

- [ ] Given the maintainer's choice, when it lands, then either a label's style cannot be written
  without a transform, with the schema regenerated, or a proposed ADR states the two meanings and
  a test cites it.
- [ ] Given a label's style without a transform in a hand-written style document, when the engine
  reads it, then it is refused as today, or cannot be read, as the choice says.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0036](../../adr/0036-a-change-of-meaning-is-a-major-format-version.md),
  [ADR-0007](../../adr/0007-harness.md); `geom_transform` and `GeomTransform` in
  `crates/style-ir/src/model.rs`; the frontend's label styles in
  `crates/mapfile/src/translate/label.rs`; `docs/semantics/label.md` (Styles); #156's style
  document decision (`gh pr view 156`).

## Blocked by

- None (can start immediately)

## Mode

with the human: the style document format and its versioning are never worked unattended
([ADR-0007](../../adr/0007-harness.md)), and the choice may be a decision for the record.
