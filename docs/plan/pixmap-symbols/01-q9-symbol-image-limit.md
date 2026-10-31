# Ticket: Bound the pixels of a symbol image (drafted specification change)

## What to build

Q-9 bounds "image size" and the size of a font file, but nothing bounds a symbol image, whose
header may claim any size: MapServer's `readPNG` and `readGIF` (`src/mapimageio.c`, 629–733 and
876–1138) allocate `width × height × 4` bytes from it before reading a pixel (668–670, 907–912). A
PNG of 65535 × 65535 compresses to a few kilobytes and would ask for 16 GiB. Draft for the
maintainer, in Q-9's list of limits:

> … the pixels of a symbol image, read from its header before it is decoded (4096 × 4096) …

and put to him the questions the spec raises beside it, each answered in the draft or left out on
purpose:

- Does the limit hold a polygon fill's tile too? `msDrawShadeSymbol` (`src/maprendering.c`,
  853–872) sizes it from the image times the style's scale, which `SIZE` alone sets, and `getTile`
  (234–236) allocates it.
- Is a pixmap file bounded in bytes, and at what? The CLI hands the engine a loader bounded at the
  32 MiB of a font today.
- Is there a bound on the decoded images one render holds, each up to 64 MiB at the limit, for a
  mapfile that names many?
- For a GIF, is the limit read from its logical screen, which `readGIF` allocates (907–912), rather
  than from its first frame?

The draft names every accepted ADR whose basis it moves, for the maintainer to confirm; the agent
judges none unaffected alone ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).

## Acceptance criteria

- [ ] Given the maintainer's decision on the wording, when it lands, then Q-9 names the limit on a
  symbol image's pixels and every further bound he chose, each with its number.
- [ ] Given the landed wording, when the tickets of this spec that implement it are read, then
  each criterion that tests a limit states the number Q-9 states.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; the spec `docs/plan/pixmap-symbols/spec.md` (Implementation
  decisions, Open questions); `docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md`,
  a drafted change of the same criterion.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7), and the resource limits are kept with
him by [ADR-0007](../../adr/0007-harness.md).
