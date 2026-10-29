# Ticket: Decide how MapServer reports its label positions

## What to build

An ADR, through `propose-adr`, revising ADR-0027: how the MapServer the noise measurement builds
reports, per case, each label's text, its position in pixels, and whether it was placed or
discarded, so that the harness can hold G-10 label by label. Candidates: an output MapServer has at
the pin, or an instrumentation kept in the repository and applied to the pinned source before the
build. The ADR states what a position is (the anchor, the box) and how it is compared within 1 px.

## Acceptance criteria

- [ ] Given the proposed ADR, when the maintainer reads it, then it names where in MapServer's
      label cache the positions are taken, from its source at the pin, and each alternative with
      why not.
- [ ] Given the ADR, then it says how the instrumentation, if any, stays tied to the pin and is
      held by the self-test of ADR-0027.

## Read first

- The spec beside this ticket; G-10; ADR-0027 and its self-test.
- MapServer at the pin: the label cache and where a label's final box is decided.

## Blocked by

- None (can start immediately)

## Mode

with the human: it decides an ADR, the maintainer's (ADR-0007).
