# Ticket: Give a label's marker bounds one owner

## What to build

MapServer's `computeMarkerBounds` and the offset `msDrawLabelCache` takes from a marker are spread
over three crates:

- The engine's `LabelMarker` (`crates/engine/src/label_styles.rs:34-43`) copies the five
  positional parameters of `label::marker_bounds` (`crates/label/src/place.rs:222-228`).
- `marker_of` (`label_styles.rs:97-124`) resolves the symbol itself and states the default anchor
  `[0.5, 0.5]` again (`label_styles.rs:114-116`), which `crates/symbolize/src/symbol.rs:489`
  holds, and scales the style's `OFFSET` by hand (`label_styles.rs:122`) as
  `symbolize::line::style_offset` does.
- `place.rs:240-246` repeats the anchor arithmetic of `symbol::anchor_shift`
  (`symbol.rs:466-478`).
- `Members::member` of the engine's cache (`crates/engine/src/labels.rs:1193-1228`) does
  `msDrawLabelCache`'s marker offset itself, and `labels.rs:1013-1015` computes a member's box
  again for the report, which `label::cache` computed when it filed the member
  (`crates/label/src/cache.rs:183`).

Let `symbolize` give a label style's marker as one value beside `label_style_ops`, of a type that
`label` declares (`label` depends on `render` and `style-ir` alone, ADR-0008, and `symbolize` may
depend on `label` only as ADR-0008 allows; where it may not, the type lives in `style-ir`). Let
`label` own the member's offsets, and let `walk_label_cache` hand its caller the box it filed. The
engine then stores and forwards. No behaviour changes.

## Acceptance criteria

- [ ] Given the suite at the base, when the marker has one owner, then it passes with every
  expected value unchanged; a unit test of `marker_of` moves with the function it tests.
- [ ] Given the workspace, when the change lands, then the default anchor and the anchor
  arithmetic are written once, outside the tests.

## Read first

- Touches: `crates/engine/src/label_styles.rs`, `crates/engine/src/labels.rs`,
  `crates/label/src/place.rs`, `crates/label/src/cache.rs`, `crates/symbolize/src/label.rs`,
  `crates/symbolize/src/symbol.rs`
- `docs/adr/0031-label-glyphs.md`, `docs/adr/0032-label-cache.md`;
  `docs/backlog/marker-size-bound-twice.md`, which concerns how the style is bound, not this.

## Blocked by

- docs/plan/revision-2026-11-13/06-layer-factors-once.md

## Mode

unattended: a refactoring decided by the suite's expected values; one revert. Stop when a test's
expected value moves, or when `label` would need a dependency ADR-0008 does not allow.
