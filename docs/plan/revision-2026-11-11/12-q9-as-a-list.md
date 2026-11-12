# Ticket: Restate Q-9 as a list of its bounds (drafted specification change)

## What to build

Q-9 of `docs/SPECIFICATION.md` is one paragraph of about 310 words that each bound has lengthened,
the last in PR #177. Two of its bounds read alike and are not one: "the total size of a mapfile
with its `INCLUDE`s and the files it names (16 MiB of text)" and "the text the defaults of
`VALIDATION` add to a load's expressions (16 MiB)", so a load holds up to 32 MiB of text, which the
sentence leaves to the reader to add. The draft below restates the sentence as a list, every value
unchanged. Only the maintainer lands it ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule
7). The draft moves no value, so no accepted ADR's basis moves; the maintainer confirms that
against the ADRs that cite Q-9 (`rg -l 'Q-9' docs/adr`).

Draft:

> - **Q-9** — Bounded resources: each limit below is a diagnostic when an input passes it, and a
>   test per limit feeds an input past it and expects the diagnostic. The two bounds on memory at
>   the end are measured by a test instead.
>   - A load: `INCLUDE` depth and cycles; the text of a mapfile with its `INCLUDE`s and the files
>     it names (16 MiB); the text the defaults of `VALIDATION` add to its expressions (16 MiB,
>     apart from that total); the diagnostics it reports (1000); the values a style document of a
>     newer minor drops (100).
>   - Files a host reads: a data file (256 MiB); a font file, which a symbol image's file shares
>     (32 MiB, apart from the load's text).
>   - Images: image size; the pixels of a symbol image, read from its header before it is decoded,
>     a GIF's from its logical screen (4096 × 4096); the pixels of the tile a polygon is filled
>     with, whatever its size or gap asks (4096 × 4096); the images one render holds decoded, its
>     fill tiles among them (256 MiB).
>   - Drawing: the features one layer draws (1,000,000); the points of the symbols one layer places
>     at its features, along its lines, or where a style's geometry transformation puts them
>     (1,000,000), the stand-in square of a style without a symbol, or of symbol 0, not counted;
>     the labels one render's label cache holds (100,000).
>   - Expressions: nesting (24 levels); the length of a text an expression reads or computes
>     (16 MiB); a regular expression's length (64 KiB) and size.
>   - Memory, each bound measured by a test and no diagnostic: a label the label cache holds keeps
>     at most 400 bytes apart from its text, and at most 32 more for each style value a feature
>     binds to it; what one render holds for its labels' texts comes to at most four times the size
>     of the input that gives them and 12 bytes for each byte of text, whatever their size.

## Acceptance criteria

- [ ] Given the maintainer's decision, when it lands, then Q-9 states every bound and value of the
  sentence at 9f836e7, and none more.
- [ ] Given the change, when `scripts/check-docs.sh` and `scripts/check-traceability.sh` run, then
  both pass.

## Read first

- Touches: `docs/SPECIFICATION.md`
- [`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7; the ADRs that cite Q-9.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change to the specification is the maintainer's
([ADR-0006](../../adr/0006-harness.md)).
