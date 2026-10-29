# Ticket: Set the tolerance of the category text in G-13

## What to build

From the noise measurement over the text cases, the maintainer picks the threshold and the
tolerance of the category text; a drafted specification change writes them into G-13, and once
signed the harness takes them and cites G-13 for that category.

## Acceptance criteria

- [ ] Given the noise report over the text cases, then the drafted change names its maximum and the
      margin the maintainer picked.
- [ ] Given a text case MapServer itself does not reproduce, then the list moves it out of scope
      with that reason.
- [ ] Given the signed change, then the harness compares the category text by its numbers and
      cites G-13 with a `Verifies:` marker.

## Read first

- G-13 and the Assumption on reproducibility; ADR-0027; ticket 08 of the conformance harness as
  the pattern.

## Blocked by

- docs/plan/labels/07-point-label-cases-in-scope.md

## Mode

with the human: it drafts a change to the specification, the maintainer's (ADR-0007).
