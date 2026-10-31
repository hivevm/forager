# Ticket: A label reads its inline feature's `TEXT` as MapServer reads it (G-10)

## What to build

An inline `FEATURE` carries its `TEXT` in the style IR
([ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md)), but the engine's labels do not
read it: [`docs/semantics/feature.md`](../../semantics/feature.md) says so, and in 10 of the 12
failing text cases of the conformance harness no label is drawn for that reason. The labels take
their text where MapServer takes it, the feature's text included, in the order of
`msShapeGetLabelAnnotation` in `src/maplabel.c` of rel-8-6-6, read there rather than assumed, and
the semantics page says what a label reads.

## Acceptance criteria

- [ ] Given a point layer with an inline feature whose `TEXT` is set and a class with a label,
  when the map renders, then the label draws that text, where MapServer's order of the label's
  `TEXT`, the class's `TEXT`, `LABELITEM`, and the feature's text puts it.
- [ ] Given the conformance harness, when it runs, then each text case this changes holds its new
  listed outcome, and the list names for each still failing what it lacks.
- [ ] Given `docs/semantics/feature.md`, when it is read, then it says what a label reads of the
  feature's text and no longer that the labels do not read it.

## Read first

- [ADR-0033](../../adr/0033-inline-features-in-the-style-ir.md); the engine's labels and their
  text; `docs/semantics/feature.md`; the conformance harness and its list `cases.txt`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the harness and the engine's tests decide its criteria; its scope is the engine's
labels and the list, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a case of the list changing its outcome; stop: after two attempts that leave
`scripts/check-all.sh` red, or when it needs a change of the style IR, the session leaves its state
in this file and hands off.
