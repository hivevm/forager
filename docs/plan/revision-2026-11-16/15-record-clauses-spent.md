# Ticket: The record drops the clauses the fidelity plan has spent, at the next normalization

## What to build

Three clauses of accepted ADRs bound only the order of the fidelity plan's work. They bind nothing
now:

- ADR-0041 *Precondition*: it must not be accepted before ADR-0042. Both are accepted.
- ADR-0048 *Order of landing*: each of `simplify`, `buffer`, and `simplifypt` enters the tree in
  the change that draws it. All three are drawn, and none is a `not_drawn` reason any more.
- ADR-0041 *Dependency* and ADR-0042 *The dependency* both state that `symbolize` alone among the
  workspace's crates depends on geo. A change to the rule would have two places to change.

ADR-0041, ADR-0047, and ADR-0048 decide one thing between them: the transform tree's nodes and the
geometry code that computes them. They are a candidate for one consolidating ADR.

The record is restated only on a human's instruction (`normalize`). This ticket keeps the
candidates until the maintainer next asks for a normalization. It changes no ADR by itself.

## Acceptance criteria

- [ ] Given the maintainer's next instruction to normalize, when the normalization restates the
  record, then the three clauses above are gone or restated, the geo rule is stated once, and the
  maintainer has decided whether ADR-0041, ADR-0047, and ADR-0048 are consolidated.

## Read first

- Touches: `docs/adr/0041-geometry-through-geo.md`,
  `docs/adr/0042-i-overlay-in-render-agg-and-geo.md`,
  `docs/adr/0047-simplifypt-ported-from-jts.md`, `docs/adr/0048-transform-tree-with-simplifypt.md`
- `docs/adr/README.md` (mechanics); the `normalize` skill.

## Blocked by

- None (can start immediately)

## Mode

with the human: only the maintainer starts a normalization and accepts the restated record.
