# Ticket: Build a label's text with `expr` and the engine's property lookup

## What to build

`text` in `crates/engine/src/labels.rs` evaluates a label's text by itself for a string, a property
as text, and their concatenation: it reads a property by its exact key, where the same file's `bind`
and `classes::FeatureProperties` match a name without case first, so with GeoJSON a `[Name]` binding
resolves and `tostring(name)` reads empty; it refuses a boolean where `value_text` gives `1` or `0`;
and it is held to none of Q-9's text bound of 16 MiB that `Evaluator::text` keeps. Build the text
with `Evaluator::text` of `crates/expr` over `FeatureProperties`, reporting its failures as
`classes::Reports` does. [ADR-0031](../../adr/0031-expression-evaluator.md) leaves a label's
expression out of its scope; this ticket uses the evaluator only for the forms `text` reads today,
so no decision moves, and a form the evaluator reads beyond them is the labels spec's.

## Acceptance criteria

- [ ] Given a GeoJSON feature with a property `name` and a label `TEXT ([NAME])` and `TEXT
  (tostring([NAME]))`, when it is drawn, then both give the same text.
- [ ] Given a label whose text concatenates a property past 16 MiB, when it is drawn, then the label
  is left out with the diagnostic of Q-9's text bound.
- [ ] Given a boolean property as a label's text, when it is drawn, then the text is what
  `value_text` gives, as for a binding.

## Read first

- [ADR-0031](../../adr/0031-expression-evaluator.md); `text`, `bind`, and `FeatureProperties` in
  `crates/engine/src/labels.rs` and `crates/engine/src/classes.rs`; `Evaluator::text` in
  `crates/expr/src/lib.rs`; the Q-9 text bound's test.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes how a resource limit of Q-9 is held, which ADR-0007 keeps with the
maintainer.
