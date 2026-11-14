# Ticket: Drop the GIF clause of Q-9 (drafted specification change)

## What to build

Q-9's *Images* bound "the pixels of a symbol image, read from its header before it is decoded, a
GIF's from its logical screen (4096 × 4096)". On 2026-11-13 the maintainer decided that Forager
does not support GIF pixmaps, and dropped the ticket for them (1feee91). `forager_encode`'s pixmap
decoder refuses every format but PNG (`decode_pixmap` in `crates/encode/src/pixmap.rs`, its test
`a_format_other_than_png_is_unsupported`). No test decides the
GIF clause, and none can. The draft below removes it, every value unchanged. Only the maintainer
lands it ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7), and confirms which accepted
ADRs cite it (`rg -il 'GIF|logical screen' docs/adr` finds ADR-0029).

Draft of the sentence:

> *Images:* image size; the pixels of a symbol image, read from its header before it is decoded
> (4096 × 4096); the pixels of the tile a polygon is filled with, whatever its size or gap asks
> (4096 × 4096); the images one render holds decoded, its fill tiles among them (256 MiB).

## Acceptance criteria

- [ ] Given the draft, when the maintainer lands it, then the change names every accepted ADR
  whose basis it moves, or says that none moves.
- [ ] Given `scripts/check-traceability.sh`, when it runs after the change, then it passes.

## Read first

- Touches: `docs/SPECIFICATION.md`
- `docs/adr/0029-pixmaps-held-once-per-render.md`

## Blocked by

- None (can start immediately)

## Mode

with the human: the specification is the maintainer's (ADR-0006).
