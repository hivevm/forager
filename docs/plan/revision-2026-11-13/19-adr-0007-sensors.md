# Ticket: Decide the sensors ADR-0007 names, a duplication sensor among them

## What to build

ADR-0007's *Sensors* name `scripts/sensor-tests-kept.sh` and no other script. The refactor tests
sensor of 6109830 runs in `scripts/check-all.sh` and CI, and the review skill names it, but the
skill's line says "the sensors the toolchain ADR names", which it is not among (6109830's doubts).
Ticket 14 would add a third. The skill names each sensor one by one, so each new one edits it
(a follow-up of 6109830 before the fold of 2026-11-13 had to add the missing line).

ADR-0007's *Follow-ups* also name "a duplication sensor once a design revision finds duplication".
This revision finds it repeatedly: the drawing of a display item written five times before 3f56d73,
a symbol's name looked up twice before c6e9e1a, C's ports before ef639d7 and 3beee16, and tickets
03, 04, 07, 10, 11, and 18 here. On 2026-11-13 the maintainer chose to add one: research a tool the
environment can carry without Node, and propose it in the ADR. jscpd needs Node, which the
environment does not carry. Run `propose-adr` with the maintainer: supersede ADR-0007's *Sensors* to
name the sensor scripts by a rule (every `scripts/sensor-*.sh` that `check-all.sh` runs), decide
whether a duplication sensor is due now and which tool, and point the review skill at that rule.

## Acceptance criteria

- [ ] Given the maintainer's decision, when it is written, then a `proposed` ADR superseding
  ADR-0007 lands in a commit of ADRs only, or the ticket records why none is due.
- [ ] Given the decision, when it is implemented, then the review skill's line holds without an
  edit for a new sensor.

## Read first

- Touches: `docs/adr/0007-rust-toolchain.md`, `.agents/skills/review/SKILL.md`,
  `.agents/skills/propose-adr/concepts/toolchain.md`, `scripts/check-all.sh`
- `docs/adr/README.md`, its mechanics of superseding.

## Blocked by

- None (can start immediately)

## Mode

with the human: an ADR, and the review skill is the template's.
