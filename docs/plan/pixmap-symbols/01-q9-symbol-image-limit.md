# Ticket: Bound symbol images, fill tiles, and decoded images (drafted specification change)

## What to build

Q-9 bounds "image size" and the size of a font file, but nothing bounds a symbol image, whose
header may claim any size: MapServer's `readPNG` and `readGIF` (`src/mapimageio.c`, 629–733 and
876–1138) allocate `width × height × 4` bytes from it before reading a pixel (668–670, 907–912). A
PNG of 65535 × 65535 compresses to a few kilobytes and would ask for 16 GiB. `msDrawShadeSymbol`
(`src/maprendering.c`, 853–872) sizes a polygon fill's tile from the image times the style's scale,
which `SIZE` alone sets, and `getTile` (234–236) allocates it. The maintainer has decided the
limits; draft them in Q-9's list:

> … the pixels of a symbol image, read from its header before it is decoded, a GIF's from its
> logical screen (4096 × 4096), the pixels of the tile a polygon is filled with (4096 × 4096), the
> images one render holds decoded (256 MiB) …

A pixmap file is read within the 32 MiB of its load path, which the draft leaves as it is: no byte
limit of its own. Each limit is a diagnostic past it, with a test per limit, as Q-9 already asks.

The draft names every accepted ADR whose basis it moves, for the maintainer to confirm; the agent
judges none unaffected alone ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).

## Acceptance criteria

- [ ] Given the maintainer's decision on the wording, when it lands, then Q-9 names the pixels of
  a symbol image (4096 × 4096, a GIF's logical screen), a polygon fill's tile (4096 × 4096), and
  the decoded images of one render (256 MiB), each with its number.
- [ ] Given the landed wording, when the tickets of this spec that implement it are read, then
  each criterion that tests a limit states the number Q-9 states.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; the spec `docs/plan/pixmap-symbols/spec.md` (Implementation
  decisions, Decisions of the maintainer);
  `docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md`, a drafted change of
  the same criterion.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7), and the resource limits are kept with
him by [ADR-0007](../../adr/0007-harness.md).
