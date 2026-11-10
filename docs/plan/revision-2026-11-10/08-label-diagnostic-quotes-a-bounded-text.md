# Ticket: Quote at most a bounded part of a label's text in its diagnostic

## What to build

Two diagnostics of a label quote the feature's whole text: `label-line-unsupported` ("the text
`{text}` holds U+…", `text_lines` in `crates/engine/src/labels.rs`) and `label-glyph-unreadable`
("its font cannot draw glyph … of `{text}`", `Unlaid::reason`). Each is reported once per label
definition and code, so a render keeps one copy of a text for every label that reads it. A text
may be 16 MiB, Q-9's bound on a text an expression reads, and a mapfile within its 16 MiB holds
hundreds of thousands of `LABEL` blocks that read the same item: a thousand such labels on one
feature with a 16 MiB text of a character at or above U+02B0 keep 16 GiB of diagnostics, from an
input of 32 MiB. The render's diagnostics are not counted against any bound, and
`label_memory.rs` measures no label that is refused. The message is also formatted for every
feature before `report_with` decides whether to keep it, so a long text costs a copy per feature
in time, though not in memory.

Quote at most a fixed number of characters of the text, such as 64, followed by an ellipsis and
the text's length in characters where it is cut; name the line by its number among the label's
lines and the character that needs a shaper, which is what
[ADR-0028](../../adr/0028-text-stack.md) asks of the diagnostic ("naming the line"); and build the
message only when it is reported. Codes stay as they are (the convention that codes are stable);
messages may be reworded.

## Acceptance criteria

- [ ] Given a point layer with many labels reading one item, and a feature whose text of 1 MiB
  holds a character at or above U+02B0, when it renders, then each label is reported once with
  `label-line-unsupported`, each message is at most a few hundred bytes and names the line's
  number and the character, and a test in `crates/engine/tests/labels.rs` states both.
- [ ] Given a font that cannot draw a glyph of a long text, when it renders, then
  `label-glyph-unreadable` quotes the text cut in the same way, decided by a test.
- [ ] Given the short texts of today's tests of `label-line-unsupported` and
  `label-glyph-unreadable`, when they run, then they pass with their codes unchanged.

## Read first

- [ADR-0028](../../adr/0028-text-stack.md); `text_lines`, `Unlaid::reason`, and `report_with` in
  `crates/engine/src/labels.rs`; the tests of both codes in `crates/engine/tests/labels.rs`; Q-9
  in `docs/SPECIFICATION.md`; the conventions on diagnostic codes and on a test that expects a
  refusal in `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

unattended: engine tests decide the messages' length and content; the scope is two messages of
the label path, and a wrong result costs one revert, noticed by the tests of both codes. It
touches none of the parts [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer, since a
message is no interface, and counts against its cap of three. Stop: when another diagnostic of the
engine turns out to quote a feature's text or property in full, the session lists it here and
hands off rather than widening the change.
