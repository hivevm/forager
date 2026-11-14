# Ticket: Warn of a style's transform that draws nothing in one place

## What to build

A class's style whose `GEOMTRANSFORM` draws nothing in a line or a polygon layer is warned of in
two crates:

- at load, by `class_labelcenter` (`crates/mapfile/src/translate/mod.rs:564-590`), for
  `labelcenter`;
- at render, by `expressions_not_drawn` (`crates/engine/src/lib.rs:817-836`), for an expression.

So a style document with `labelcenter` gets no warning, and a mapfile's expression is warned of
only when it renders. The frontend's `style(b, of_label)` and `geom_transform(d, of_label)`
(`crates/mapfile/src/translate/style.rs:28,119,195,227`) take a flag only to keep that warning off
a label's style, and `label_style` reads the block for its `GEOMTRANSFORM` again
(`crates/mapfile/src/translate/label.rs:166-170`). Warn of both in the engine, where it holds for
every document, and let `geom_transform` return the transform without warning, so the flag and the
second read go.

## Acceptance criteria

- [ ] Given a style document whose line layer's style has `geom_transform: labelcenter`, when it
  renders, then the layer reports the style undrawn, by a test that asserts the code.
- [ ] Given a mapfile of the same, when it loads and renders, then the warning is reported once.
- [ ] Given `crates/mapfile/src/translate/style.rs`, when the change lands, then no function takes
  `of_label`.

## Read first

- Touches: `crates/mapfile/src/translate/mod.rs`, `crates/mapfile/src/translate/style.rs`,
  `crates/mapfile/src/translate/label.rs`, `crates/engine/src/lib.rs`
- `docs/CONVENTIONS.md`, *Diagnostic codes are kebab-case and stable*.

## Blocked by

- docs/plan/revision-2026-11-13/01-geom-transform-expression-in-ir.md

## Mode

unattended once ticket 01 has landed: the tests above decide it; one revert. The load's
`unsupported-value` for `labelcenter` gives way to the engine's `geomtransform-not-drawn`, which a
mapfile checked by `forager read` alone no longer reports: say so in the hand-off. Stop when ticket
01's transform tree leaves no expression variant to warn of.
