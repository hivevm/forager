# Ticket: Let a second design revision on the same day reset the revision sensor's count

## What to build

`scripts/sensor-revision-due.sh` counts from the oldest commit whose diff of
`docs/ARCHITECTURE.md` adds the `Last design revision` line with its date, so that a change of
the number alone does not reset it. A second revision on the same day cannot move the line: it
already reads that date. This revision of `style-document-versions` ran on 2026-10-24, the day
of the revision after the browser tracer (`73be508`), and after it the sensor still reports 27
changes and DUE, counting the work this revision covered. It happened again on 2026-10-31: the
second revision of that day (`docs/plan/revision-2026-10-31b/`) found the line already reading
2026-10-31, the sensor anchored on the first revision's commit `7273150`, and after its own commit
the sensor still reports the 26 changes it covered, and DUE.

The line, or the sensor, distinguishes the revision that ran from the one before it on the same
day — for example a date with a time, or the sensor taking the youngest commit that adds the line
when the line before it differed in more than the number. The choice changes the line of ADR-0006
and is the human's; the template carries the sensor, so the fix likely belongs there first.

## Acceptance criteria

- [ ] Given a fixture history with two revisions on one day and changes between them, when the
  sensor runs after the second, then it counts only the changes after the second.
- [ ] Given a change of the number alone, when the sensor runs, then the count does not reset.
- [ ] Given `scripts/test-sensor-revision-due.sh`, when it runs, then its existing cases pass.

## Read first

- [ADR-0006](../../adr/0006-feature-layer.md); `scripts/sensor-revision-due.sh` and its
  self-test.

## Blocked by

- None (can start immediately)

## Mode

with the human: it changes the line ADR-0006 decides, and the sensor is the template's.
