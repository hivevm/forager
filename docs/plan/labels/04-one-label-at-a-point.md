# Ticket: One label drawn at a point

## What to build

The tracer bullet through the text path: the engine draws a point layer's label, its text from
`TEXT`, the class, or the layer's `LABELITEM`, shaped and drawn in its font, size, and colour,
centred on the point, through the reference renderer. Forager's own reference image pins it.

## Acceptance criteria

- [ ] Given a point layer with a label of a TrueType font, when the engine renders, then the text is
      drawn centred on each point in the label's colour, and the image matches Forager's reference.
- [ ] Given a `LABELITEM` and features with that property, then each label shows its feature's
      value; a feature without it gets no label.
- [ ] Given a label whose font did not load, then it is not drawn and the diagnostic of ticket 03
      stands; nothing panics.
- [ ] Given the same input natively and on `wasm32-unknown-unknown`, then the bytes are the same.

## Read first

- The ADR of ticket 01; ADR-0015; the engine's render tests, which this copies.
- The glossary's *Label cache*; the conventions on tests and on readers that fail closed.

## Blocked by

- docs/plan/labels/03-fonts-reach-the-engine.md
- docs/plan/revision-2026-10-24/29-text-as-mapserver-evaluates-it.md

## Mode

unattended: the engine's tests decide every criterion; the change is the engine and the reference
renderer, undone by one revert; nothing ADR-0007 keeps with the maintainer, as the crates are
decided by ticket 01. It counts against the cap of three open agent branches of ADR-0007. Stop
when a crate's API cannot do what the ADR of ticket 01 decided, and hand off.
