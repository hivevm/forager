# Ticket: Carry attribute bindings in styles, and MapServer's colour range

## What to build

`SIZE [VAL]`, `WIDTH [w]`, `COLOR [c]`, `ANGLE [a]`, and the other style properties MapServer lets
bind load into the IR as bound values — a literal or an expression of the ADR-0019 tree, a
number property wrapped in `to_number` — instead of failing with `invalid-value`. A colour accepts
−255…255 per channel, as MapServer's `loadColor` does; −1 still means no colour, and what another
negative channel draws in MapServer is determined from its renderer and resolved at import, so the
IR holds only 0…255. Writes or extends the
`docs/semantics/` pages of the properties it touches.

## Acceptance criteria

- [ ] Given `STYLE SIZE [VAL] END`, when the mapfile loads, then the style's size is
  `to_number` of the property `VAL` and no error is reported.
- [ ] Given `WIDTH [w]`, `ANGLE [a]`, and `COLOR [c]`, when they load, then each is a bound value
  of its property, the colour one read as a colour string.
- [ ] Given `COLOR -255 -255 -255` and `COLOR 300 0 0`, when they load, then the first loads with
  no error, as what MapServer draws for it (stated on the semantics page with its source), and the
  second is an `invalid-value` error, as in MapServer.
- [ ] Given a document with a bound size, when it is written as YAML and JSON and read back, then
  it is equal.

## Read first

- [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0014](../../adr/0014-style-document-serde.md);
  the spec next to this ticket (*Bound values*, *Colours*).
- `msautotest` files that fail today on bindings: `.scratch/msautotest-survey/REPORT.md`, if
  present; otherwise grep the suite for `SIZE [`.
- MapServer `rel-8-6-6`: `loadColor` and `loadStyle` in `src/mapfile.c` (bindings, the colour
  range), and the renderer's use of a negative channel, for the semantics pages.

## Blocked by

- docs/plan/ir-completeness/01-class-expressions-and-layer-filters.md

## Mode

with the human: it changes the style document format ([ADR-0007](../../adr/0007-harness.md)).
