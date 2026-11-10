# Ticket: Measure and explain what a long label text costs relative to its size

## What to build

The review fixes of revision-2026-10-31/03 measured what a render keeps of a label's long text
over a text of one word, on a MultiPoint, with `a_long_text_is_held_once_for_all_its_points`
(`crates/engine/tests/label_memory.rs`). Relative to the text's own bytes, the cost differs with the
number of points and, at 2,000 points, with the text's size:

| Points | Text | Kept, relative to the text |
|---|---|---|
| 2,000 | 16 KiB | 5.98× |
| 2,000 | 64 KiB | 8.25× |
| 10,000 | 16 KiB | 5.00× |
| 10,000 | 64 KiB | 5.00× |

A first run at 2,000 points also read 10.2× at 128 KiB and 11.6× at 256 KiB. The rise may be partly
an effect of subtracting two peaks, the render with a long text and the one with a word, rather
than a cost of the text. A comment once put the cost down to four-byte characters and glyphs, which
was inferred from the code and never measured. The maintainer's bound of four times the input holds
at the test's sizes (10,000 points, 16 KiB, 0.72× the input) because coordinates outweigh the text.
At 2,000 points and 64 KiB the text held once came to 6.4× the input (540,420 bytes for an input of
84,429), so the bound does not hold where a text dominates its input.

Measure what makes up the cost, and say whether it grows with the text's size and why. Fix it
where a list or a copy is the cause and the fix is small. Otherwise record what was found, and put
to the maintainer whether the bound holds for an input of a long text.

## Acceptance criteria

- [ ] Given texts of 16, 64, 128, and 256 KiB at 2,000 and at 10,000 points, when what a render
  keeps is measured, then the ticket's result names what makes up the cost, by list, with numbers,
  and whether subtracting the peaks explains the rise.
- [ ] Given a cause that a small change removes, when it lands, then a case in `label_memory.rs`
  decides it. Given none, then the finding goes to the maintainer, and the 4× bound is left as it
  is.

## Read first

- `crates/engine/tests/label_memory.rs`; `crates/engine/src/labels.rs`; `crates/label`'s text path;
  `docs/SPECIFICATION.md` (Q-9); `docs/CONVENTIONS.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: what the bound means for an input of a long text is the maintainer's
([ADR-0007](../../adr/0007-harness.md)). Stop: if the cause needs more than a small change, record
the measurement here and hand off.
