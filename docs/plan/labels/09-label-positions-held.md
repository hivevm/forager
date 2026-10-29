# Ticket: Hold each label's position and status against MapServer's (G-10)

## What to build

The noise measurement's MapServer reports each label's text, position, and placed or discarded
status, as the ADR of ticket 02 decides; the harness compares Forager's labels with them case by
case and reports, per case, the labels placed or discarded otherwise and those more than 1 px off.

## Acceptance criteria

- [ ] Given a text case in scope, when the harness runs, then it compares every label of it with
      MapServer's and names each that differs, with the distance.
- [ ] Given a case whose labels all match within 1 px with the same statuses, then it holds G-10,
      and the harness cites G-10 with a `Verifies:` marker for the cases the list marks.
- [ ] Given the self-test of ADR-0027, then it holds the instrumentation the ADR of ticket 02
      decides.

## Read first

- G-10; the ADRs of ticket 02 and ADR-0027; the harness and its list.

## Blocked by

- docs/plan/labels/02-label-positions-from-mapserver-adr.md
- docs/plan/labels/07-point-label-cases-in-scope.md

## Mode

with the human: it changes the build of the instrument ADR-0027 decides, and marks which cases hold
a criterion of the specification.
