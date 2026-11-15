# Ticket: Propose the ADR on what style-ir holds beyond the style document

## What to build

ADR-0008's row for `style-ir` reads "data model, diagnostics, document format, CQL2 tree". The
crate now holds computed values that are no part of the document:

- the marker a label's `labelpoint` style gives, and the anchor arithmetic
  (`crates/style-ir/src/marker.rs`);
- a map's fonts and the one reader of a font list (`font.rs`);
- the geometry of a feature (`geometry.rs`), which the ports deliver and `symbolize` draws.

C's and MapServer's small ports (`c.rs`) live there too, but ADR-0017 decides that. The others
live there because ADR-0008 gives `symbolize` no edge to `label` and `mapfile` no edge to
`label`. Only `docs/ARCHITECTURE.md` records it. A reader without that context would take
`marker.rs` in the crate of the style document for a mistake, and the next shared value has no
rule for where it goes. Run `propose-adr` with the maintainer. It decides what `style-ir` may hold
beside the document, or moves the shared values to a crate of their own with ADR-0008's edges.

## Acceptance criteria

- [ ] Given the maintainer's answers, when the ADR is submitted, then it is `proposed`, in a
  commit of ADRs only, and names the modules of `style-ir` it admits or moves.

## Read first

- Touches: `docs/adr/`, `docs/adr/README.md`
- `docs/adr/0008-ports-and-adapters-crates.md` whole, `docs/adr/0017-c-ports-in-style-ir.md`;
  `docs/ARCHITECTURE.md`, `style-ir`; the skill `propose-adr`, its catalogue entry for the shape
  of the code.

## Blocked by

- None (can start immediately)

## Mode

with the human: an ADR is developed with the human
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 3).
