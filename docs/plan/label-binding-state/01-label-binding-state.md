# Ticket: A bound label position or alignment that names nothing keeps the one MapServer keeps

## What to build

A label whose `POSITION`, or whose `ALIGN` over several lines, is bound to an attribute is drawn
for every feature, also where the feature's value names no position or alignment. MapServer's
`bindLabel` then leaves the label's value as it stood: the one the previous feature bound, of
whatever class of the layer, as `msBindLayerToShape` binds every class's labels for each shape;
or, before any feature bound one, the one the mapfile's load left. Today the engine leaves such a
label of such a feature undrawn with the warning `label-binding-unsettled`.

The style IR carries the value the load leaves beside the binding: for `POSITION`, `MS_BINDING`
where the binding is the label's last `POSITION`, else the last keyword; for `ALIGN`, a keyword
before or after the binding, else `MS_ALIGN_DEFAULT`. The engine follows `bindLabel`'s state
across the features and the classes of a layer, in the order MapServer binds them, so that the
warning goes.

## Acceptance criteria

- [ ] Given a mapfile whose label has `POSITION` bound and a keyword `POSITION` after it, when it
      is loaded, then the IR carries the binding and the keyword's position as the loaded value;
      given the binding last, then it carries `MS_BINDING`'s placement.
- [ ] Given two features, the first binding `ul`, the second a value that names no position,
      when rendered, then both labels stand at UL, with no diagnostic.
- [ ] Given a first feature whose value names no position, when rendered, then its label stands
      at the loaded value's position.
- [ ] Given features of two classes, the second class's label bound, then a feature of the first
      class moves the state the second class's label keeps, as `msBindLayerToShape` moves it.
- [ ] Given the same for `ALIGN` on a text of several lines, then the same holds.
- [ ] Given a style document of an earlier version, then it loads as before (ADR-0020).

## Read first

- ADR-0007 (the style document format stays with the maintainer), ADR-0014, ADR-0020 (the
  document's compatibility), ADR-0028.
- The docs of `Label::position` and `Label::align` in the style IR, and
  docs/semantics/binding.md and docs/semantics/label.md.
- MapServer at the pin: `loadLabel`, src/mapfile.c (`POSITION`, `ALIGN` with a binding);
  `bindLabel` and `msBindLayerToShape`, src/maputil.c; where `msDrawShape` binds a shape.
- The engine's binding of label properties and its test
  `a_bound_position_or_alignment_that_names_none_is_not_drawn_yet`, which this ticket replaces.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the style document format, which ADR-0007 keeps with the maintainer,
and decides how the IR holds a loaded value beside a binding.
