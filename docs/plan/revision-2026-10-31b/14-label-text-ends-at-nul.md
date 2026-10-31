# Ticket: End a label's text at its first NUL, as MapServer's C string ends

## What to build

MapServer holds a label's text as a C string, so a feature's text `"ab\0cd"` draws `ab` and
`"\0abc"` draws nothing; #140 made the mapfile frontend read an expression only up to its first NUL
for the same reason. The engine treats a NUL in a label's text three ways:
`text` and `inline::feature_text` (`crates/engine/src/labels.rs`, `inline.rs`) return the whole
string, the layout lays it out NUL and all, `text.is_empty()` does not stop `"\0abc"`, while
`same_text` compares `c_bytes`, which ends at the NUL, under `MINDISTANCE`. Read in MapServer's
source at the pin where a label's text comes from (`msShapeGetLabelAnnotation`,
`src/maputil.c`; the shape's `text`; the attribute values of the sources), confirm there that the
text ends at its first NUL on each path, and end it there once, where the engine takes the text, so
that drawing, emptiness, `MINDISTANCE`, and the report `Rendered.labels` read the same text.

## Acceptance criteria

- [ ] Given a point with a label whose text is bound to a property `"ab\0cd"`, when rendered, then
  the label draws `ab` and `Rendered.labels` reports `ab`.
- [ ] Given a property `"\0abc"`, when rendered, then no label is drawn, as for an empty text.
- [ ] Given an inline feature whose `TEXT` holds a NUL, when its label reads it, then the same
  holds.
- [ ] Given the labels' tests and the harness, when they run, then they pass unchanged.

## Read first

- `crates/engine/src/labels.rs` (`text`, `c_bytes`, `same_text`), `crates/engine/src/inline.rs`;
  MapServer at the pin: `msShapeGetLabelAnnotation` (`src/maputil.c`), `msDrawLabelCache`
  (`src/mapdraw.c`); the entry on ported behaviour in `docs/CONVENTIONS.md`; #140's doubt 3.

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's tests decide its criteria; its scope is where the engine takes a label's
text, and a wrong result costs one revert; it touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer and counts against its cap of three;
a wrong result shows as a moved label in the harness; stop: when MapServer's source does not end the
text at the NUL on one of its paths, the session records the path in this file and hands off.
