# Ticket: A label's EXPRESSION is carried, or refuses the label, instead of being ignored

## What to build

Found by area 5 of the port review (revision ticket 23, #70). A `LABEL` may carry an
`EXPRESSION` (`loadLabel`, `src/mapfile.c:1786` at `rel-8-6-6`), evaluated against the layer's
`LABELITEM` in `msGetLabelStatus` (`src/maputil.c:987`): a label whose expression does not match
is not drawn. The frontend warns and ignores it, so the label is drawn where MapServer skips it,
which goes against the convention *A reader that skips what it cannot read fails closed*.

The label carries its expression as a condition the engine applies, read as a class's
`EXPRESSION` is read (with `LABELITEM` where a class's takes `CLASSITEM`), and the label's
condition joins the IR's list of conditions. This adds an optional field to the style document,
a minor change before the first release.

## Acceptance criteria

- [ ] Given `LABELITEM "n" CLASS LABEL EXPRESSION "x" END END`, when it loads, then the label's
  condition is the tree of `[n] = "x"`, and no warning names it.
- [ ] Given a newer minor's document whose label condition does not read, when it is read, then
  it is kept as one that never matches.
- [ ] Given `docs/semantics/label.md`, when it is read, then it states the condition with its
  source.

## Read first

- [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0020](../../adr/0020-style-document-compatibility.md);
  `crates/mapfile/src/translate/label.rs`, `crates/style-ir/src/model.rs` (`CONDITIONS`).
- MapServer `rel-8-6-6`: `loadLabel`, `msGetLabelStatus`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a field to the style document ([ADR-0007](../../adr/0007-harness.md)).
