# Ticket: G-7 leaves the specification until a source needs pushdown

## What to build

A drafted change to the specification that removes G-7, handed to the maintainer to land. No source
of Forager pushes a filter down, so G-7 is a criterion no test can cite, and the traceability check
lists it as pending for as long as it stands. Once the change lands, the check lists only what is
really pending.

The draft:

- deletes the item `7. **G-7** — When a filter is evaluated with and without pushdown, …` from
  *Goals / Success Criteria*, and keeps the numbers of G-8 to G-14. The specification already says
  that identifiers are never reused and only a normalization renumbers them; the next normalization
  closes the gap;
- names in its commit body and hand-off the accepted ADRs whose basis it moves, as
  [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules) rule 7 asks:
  - ADR-0008, whose *Context* gives pushdown (G-7) as one reason the ports and the adapters see the
    CQL2 tree. The decision still stands on the *Strategy* and the *Constraints*;
  - ADR-0015, whose *Context* asks that expressions evaluate the same with and without pushdown
    (G-7), and whose follow-up is pushdown (G-7);
  - ADR-0016, whose *Out of scope*, *Consequences*, and follow-up name the pushdown of G-7.

  The three are accepted and immutable. They keep the text `G-7` until the next normalization
  restates them. The draft says so and supersedes none of them, because no decision of theirs rests
  on G-7 alone. If the maintainer judges one does, a successor ADR goes on its own branch;
- keeps the glossary's **Pushdown**, which the three ADRs still use;
- leaves *Non-Goals* alone, unless the maintainer wants pushdown written there.

Checked on 2026-11-15, with G-7's item deleted from a copy of the specification:
`scripts/check-docs.sh` passes, and `scripts/check-traceability.sh` passes with 22 criteria and
lists only G-14 as pending. Neither check refuses a gap in the `G-n` sequence. Neither reads the
identifiers ADRs cite in prose. No `Verifies: G-7` marker exists in the tree. So the gap needs no
change to a check.

## Acceptance criteria

- [ ] Given the drafted specification, when `scripts/check-docs.sh` and
  `scripts/check-traceability.sh` run, then both pass and the traceability check no longer lists
  G-7.
- [ ] Given the draft's commit, when the maintainer reads it, then it names ADR-0008, ADR-0015, and
  ADR-0016 as the ADRs whose basis it moves, and says why none is superseded.
- [ ] Given the specification after the change, when `scripts/check-traceability.sh` runs, then it
  no longer lists G-7 as pending.

## Read first

- Touches: `docs/SPECIFICATION.md` (*Goals / Success Criteria*, the paragraph above the list)
- `docs/adr/0008-ports-and-adapters-crates.md` (*Context*), `docs/adr/0015-expressions-as-cql2.md`
  (*Context*, *Consequences*), `docs/adr/0016-expression-evaluation.md` (*Out of scope*,
  *Consequences*)
- `docs/GLOSSARY.md` (**Pushdown**)
- `scripts/check-traceability.sh` (its header)
- Seam: the two checks

## Blocked by

- None (can start immediately)

## Mode

with the human: a change to the specification is the human's to land
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules) rule 7, ADR-0006). The session drafts the change
in one commit, runs both checks, and stops at the hand-off.
