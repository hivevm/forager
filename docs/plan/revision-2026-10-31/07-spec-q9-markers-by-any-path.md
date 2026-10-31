# Ticket: Bound the markers a layer places by any path (drafted specification change)

## What to build

Q-9 bounds "the points of the symbols one layer places along its lines (1,000,000)". The engine's
budget also takes the placements of a style's `GEOMTRANSFORM`, which Q-9 does not name, and a point
layer's markers take nothing from it: a feature of many vertices in a point layer with a large
vector symbol draws without limit, the feature limit of Q-9 notwithstanding. Draft for the
maintainer:

> … the points of the symbols one layer places at its features, along its lines, or where a
> style's geometry transformation puts them (1,000,000) …

It moves the basis of no accepted ADR the revision found; the maintainer judges that, not the agent
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7). It belongs beside ticket 06 of the
revision of 2026-10-24, which drafts the feature and data-file limits of the same criterion, and may
land with it.

## Acceptance criteria

- [ ] Given the maintainer's decision on the wording, when it lands, then Q-9 names what the marker
  budget covers, and the test of the limit cites it.

## Read first

- Q-9 in `docs/SPECIFICATION.md`;
  `docs/plan/revision-2026-10-24/06-q9-feature-and-source-limits.md`; `MarkerBudget` in
  `crates/engine/src/line_marker.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).
