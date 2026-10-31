# Ticket: Narrow the style IR's fields that only one side understands

## What to build

Two fields of the style IR are wider than what writes them and what reads them. `InlineFeature.text`
is written by the frontend and read by nothing, without a warning, so a feature's `TEXT` is dropped
silently ([ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md) defers it to the labels).
`Label.shadow_color` is a `Bound<Rgba>`, yet the frontend refuses a binding for `SHADOWCOLOR` and
the engine reports the field as not supported. Warn on a feature's `TEXT` until the labels read it,
and narrow `shadow_color` to a colour while the format is unreleased
([ADR-0020](../../adr/0020-style-document-compatibility.md)'s freeze).

## Acceptance criteria

- [ ] Given a `FEATURE` with `TEXT`, when it is loaded, then a warning names the keyword as not
  carried over.
- [ ] Given the derived schema, when `crates/style-ir/tests/schema.rs` runs, then the checked-in
  schema is regenerated in the same change and `shadow_color` is a colour.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md),
  [ADR-0036](../../adr/0036-a-change-of-meaning-is-a-major-format-version.md);
  `crates/style-ir/src/model.rs`; `crates/mapfile/src/translate/feature.rs` and `label.rs`;
  `crates/engine/src/labels.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document format, which ADR-0007 keeps with the maintainer.
