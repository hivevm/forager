# Ticket: A label sits and looks where MapServer puts it

## What to build

A label honours `POSITION` (the nine positions, and `AUTO` for a point), `OFFSET` in pixels,
`ANGLE`, `ALIGN` and `WRAP` for several lines, `SIZE` in points scaled as MapServer scales it, and
`OUTLINECOLOR` with `OUTLINEWIDTH`, each as MapServer computes it at the pin, bound or literal.

## Acceptance criteria

- [x] Given a label at each of the nine positions, when rendered, then its box stands where
      MapServer's computation at the pin puts it, to the pixel, in a test of the placement alone.
- [x] Given an `OFFSET`, an `ANGLE`, and a wrapped text with each `ALIGN`, then the same holds.
- [x] Given an outline, then the text is drawn with it, and the image matches Forager's reference.

## Read first

- The spec beside this ticket; MapServer at the pin, where a label's point and box are computed.
- The engine's render tests and the tests of ticket 04.

## Blocked by

- docs/plan/labels/04-one-label-at-a-point.md
- docs/plan/revision-2026-10-24/26-label-bindings-as-mapserver-binds-them.md

## Mode

unattended: tests of the placement and the engine decide every criterion; undone by one revert;
nothing ADR-0007 keeps with the maintainer. It counts against the cap of ADR-0007. Stop when a
property's meaning in MapServer needs a change of the style IR, and hand off.

## Where it stands

Stopped at the Mode's stop. Every criterion is met for literal values and for bindings whose
value MapServer reads (`crates/label/src/place.rs`, `crates/label/tests/place.rs`, the layout and
the engine's tests in `crates/label/tests/text.rs` and `crates/engine/tests/labels.rs`, the
reference `label_wrapped_outlined.png`).

What is left needs a decision, and maybe a change of the style IR: a bound `POSITION`, or a bound
`ALIGN` of a text of several lines, whose value names nothing. `bindLabel` then keeps the value
the label held before: the previous feature's (of any class of the layer, as
`msBindLayerToShape` binds every class's labels for each shape), or on the first feature the one
the load left, which the IR does not carry (see the docs of `Label::position` and
`Label::align`). The engine leaves such a label of such a feature undrawn with the diagnostic
`label-binding-unsettled`. Settling it means deciding whether the IR carries the loaded value and
whether the engine follows `bindLabel`'s state across features; then the diagnostic goes, with a
test of a feature after one whose value matched.
