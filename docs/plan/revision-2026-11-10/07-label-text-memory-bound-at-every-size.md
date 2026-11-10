# Ticket: Hold what a render keeps of its labels' texts to a bound that holds at every size

## What to build

`a_long_text_is_held_once_for_all_its_points` in `crates/engine/tests/label_memory.rs` (8d681cc)
holds what a long label text adds over a text of one word to four times the input's size, the
GeoJSON the source parses. That bound cannot hold where the text outweighs the coordinates: laid
out, a line is held as its characters (`lines` in `crates/label/src/layout.rs`, 4 bytes for each
byte of an ASCII text) and as its glyph indices (`Font::glyphs`, 4 bytes), and the layout keeps
its lengths and its lines beside them. The case of 2,000 points and 64 KiB holds 5.0 times the
text against a bound of about 5.15 times it, a margin of 3 %; at fewer points or a longer text no
copy-free layout fits under four times the input. The test's doc comment leaves that as the
maintainer's question.

The maintainer decided on 2026-11-10: the bound becomes four times the input plus a fixed amount
per byte of label text, about 12 bytes (its characters, its glyph indices, and its layout), and it
holds at every size. Change the assertion to that bound, its constant named once in the test with
the reason for each of its bytes, and add cases where the text far outweighs the coordinates, so
that the test decides it at every size and not only where coordinates dominate. A copy of the text
per point must still fail it, as it fails today. Rewrite the test's doc comment for the new bound,
dropping the open question.

The bound is a sentence of Q-9's in substance, and the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7). Q-9 says today: "A label the label
cache holds keeps at most 400 bytes apart from its text, and at most 32 more for each style value
a feature binds to it; a test measures it." The drafted wording, for the maintainer to land or
reword in a change of their own, appends after it:

> What one render holds for its labels' texts comes to at most four times the size of the input
> that gives them and 12 bytes for each byte of text, whatever their size; a test measures it.

The accepted ADRs read for what this moves: ADR-0035 (the label cache's grid and its bound of
100,000 labels) and ADR-0028 (the text stack); neither decides a bound on a text's memory, so none
is moved as far as this revision sees. The maintainer confirms that when landing the wording.
This ticket does not edit `docs/SPECIFICATION.md`; until the wording lands, the test's doc comment
cites the maintainer's decision of 2026-11-10.

## Acceptance criteria

- [ ] Given the cases of `a_long_text_is_held_once_for_all_its_points` today, 10,000 points with
  16 KiB and 2,000 points with 64 KiB, with and without `MINDISTANCE`, when the test runs, then
  each holds what the text adds within four times the input plus the fixed amount per byte of
  text, the constant 12 or the smallest whole number at or below 12 that the measurement allows.
- [ ] Given a case whose text outweighs its coordinates many times over, such as 10 points with
  1 MiB and 2 points with 4 MiB, with and without `MINDISTANCE`, when the test runs, then it holds
  the same bound.
- [ ] Given the engine with the text copied once per point, as it was before 100f402, when the
  test runs, then it fails; the pull request states that it did so, by a temporary change that is
  not committed.
- [ ] Given the change, when `scripts/check-rust.sh` runs, then it passes, and the doc comment of
  `label_memory.rs` and of the test state the new bound, its bytes per byte of text, and the
  maintainer's decision of 2026-11-10.

## Read first

- [ADR-0035](../../adr/0035-label-cache-grid-and-bound.md),
  [ADR-0028](../../adr/0028-text-stack.md); Q-9 in `docs/SPECIFICATION.md`;
  `crates/engine/tests/label_memory.rs`; `lines` and `Layout` in `crates/label/src/layout.rs`,
  `layout` in `crates/engine/src/labels.rs`; the message of 8d681cc; the conventions in
  `docs/CONVENTIONS.md` on memory per label and on a test holding a value it states.

## Blocked by

- None (can start immediately)

## Mode

unattended: the memory test decides the bound, the scope is that test and, where a case shows a
copy, the label path that makes it; a wrong result costs one revert, and the run of
`scripts/check-rust.sh` proves it. It touches none of the parts
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer, since the Q-9 wording is only
drafted here, and counts against its cap of three. Stop: when a case needs more than 12 bytes per
byte of text, the session looks for the copy that costs it within the session; if none is found,
it writes the measurements per case into this file and hands off, as the constant is then the
maintainer's to set again.
