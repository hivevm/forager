# Ticket: Shorten the C-type convention to what the lint cannot see

## What to build

The convention *A port holds a value in the C type MapServer holds it in, at the step it does*
(`docs/CONVENTIONS.md`, about line 96) asks two things: hold the value in C's type, and cut it at
the step C cuts it. Since ef639d7, ADR-0036 and the lint `cast_possible_truncation`, denied in
`engine`, `label`, and `symbolize`, decide the first: a bare `as` fails, and `int_cast` is the one
`(int)`. The convention's own rule says that a convention a check can decide is the check. The
second half did not prevent the mistake of ef639d7, `glyph_size` rounded where C cuts to an `int`
three times, a must-fix of its review. Shorten the entry to the step, name that mistake as its
evidence, and point at ADR-0036 for the type. It is a drop of part of a rule, which the design
revision files and does not land
([`AGENTS.md` §5](../../../AGENTS.md#5-quality-bar--definition-of-done)).

## Acceptance criteria

- [ ] Given the entry, when it is shortened, then it names no `as`, `int_cast`, or lint, and
  `scripts/sensor-plain-rules.sh` reports nothing on the lines it adds.

## Read first

- Touches: `docs/CONVENTIONS.md`
- `docs/adr/0036-c-ports-in-style-ir.md`

## Blocked by

- None (can start immediately)

## Mode

unattended, on the maintainer's instruction of 2026-11-13 to implement every open ticket, the
smallest variant chosen and listed in the pull request, which the maintainer reviews: shorten the
entry to the step, as written. Stop when that variant cannot hold.