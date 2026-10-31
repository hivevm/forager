# Ticket: A label's text breaks at MAXLENGTH as MapServer breaks it

## What to build

A label with a positive `MAXLENGTH` is laid out as `textlayout.c` lays it out at the pin: when its
text has more glyphs than `MAXLENGTH`, each `WRAP` character or U+200B met once the current line
holds `MAXLENGTH` glyphs becomes a line break; a text no longer than `MAXLENGTH` is laid out as
written. Today the engine reports such a label as not drawn. The text cases that only lacked
`MAXLENGTH` (`label_latin_align_*` of `labels-alignment`, `lots_of_text`) hold their new outcome.

## Acceptance criteria

- [ ] Given a label `MAXLENGTH 5 WRAP ' '` and the text `the big complex_shape`, when it is laid
  out, then its lines are `the big`, `complex_shape`, as textlayout.c breaks them (a test whose
  expected lines are worked out from the C, not from the code).
- [ ] Given a text no longer than `MAXLENGTH`, then it is laid out as written; given `MAXLENGTH`
  without `WRAP`, then only U+200B breaks.
- [ ] Given the harness, then each case this changes holds its new listed outcome and, where its
  labels match MapServer's record, the mark `labels-hold`; the 5 marked cases stay marked.

## Read first

- `textlayout.c` at the pin (`msLayoutTextSymbol`, the `maxlength` branch); ADR-0028; the engine's
  labels and `crates/label`'s layout; the harness's list and `conformance/labels/`.

## Blocked by

- None (can start immediately)

## Mode

unattended: the layout tests and the harness decide its criteria; its scope is the label layout and
the list, a wrong result costs one revert; it touches none of the parts ADR-0007 keeps with the
maintainer and counts against its cap of three; stop: after two attempts that leave
`scripts/check-all.sh` red, or when a case that passed before falls, the session leaves its state in
this file and hands off.
