# Ticket: Narrow the style IR's field that only one side understands

## What to build

A field of the style IR is wider than what writes it and what reads it. `Label.shadow_color` is a
`Bound<Rgba>`, yet the frontend refuses a binding for `SHADOWCOLOR` and the engine reports the
field as not supported. Narrow `shadow_color` to a colour while the format is unreleased
([ADR-0020](../../adr/0020-style-document-compatibility.md)'s freeze). `InlineFeature.text`, the
other field this ticket named, is read since the labels take an inline feature's `TEXT`
(`crates/engine/src/inline.rs`, `feature_text`).

## Acceptance criteria

- [ ] Given the derived schema, when `crates/style-ir/tests/schema.rs` runs, then the checked-in
  schema is regenerated in the same change and `shadow_color` is a colour.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md),
  [ADR-0036](../../adr/0036-a-change-of-meaning-is-a-major-format-version.md);
  `crates/style-ir/src/model.rs`; `crates/mapfile/src/translate/label.rs`;
  `crates/engine/src/labels.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document format, which ADR-0007 keeps with the maintainer.
