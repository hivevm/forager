# Ticket: A label's PRIORITY may be an expression

## What to build

`PRIORITY` in MapServer at the pin takes a number, an attribute binding, or an expression, which is
evaluated per feature and bounded to 1..10. The frontend already reads the expression into the
style IR (`Bound::Expr`), but the engine refuses a label whose property is an expression, so
`label_priority_expression` caches none of its labels. Evaluate a `PRIORITY` expression per feature
through `expr` (ADR-0031), as MapServer evaluates and bounds it, and cache the label at that
priority; the other properties that refuse an expression keep refusing it.

## Acceptance criteria

- [ ] Given `PRIORITY ([n] + 1)` on a label, when a feature with `n = 4` is labelled, then the label
  is cached at priority 5, and a result outside 1..10 is bounded as MapServer bounds it (values
  read from the C, not from the code).
- [ ] Given the harness, then `label_priority_expression` holds its new listed outcome and records
  comparison; the 5 marked cases stay marked.

## Read first

- `src/maputil.c` (`bindLabel`) and `src/maplabel.c` at the pin, where a label's priority is
  evaluated and bounded; ADR-0031; the engine's labels (`prop`, `fixed`).

## Blocked by

- None (can start immediately)

## Mode

unattended: the engine's label tests and the harness decide its criteria; its scope is the engine's
labels, a wrong result costs one revert; it touches none of the parts ADR-0007 keeps with the
maintainer and counts against its cap of three; stop: after two attempts that leave
`scripts/check-all.sh` red, or when the style IR would have to change, the session leaves its state
in this file and hands off.
