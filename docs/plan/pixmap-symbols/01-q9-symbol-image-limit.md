# Ticket: Bound symbol images, fill tiles, and decoded images (drafted specification change)

## What to build

Q-9 bounds "image size" and the size of a font file, but nothing bounds a symbol image, whose
header may claim any size: MapServer's `readPNG` and `readGIF` (`src/mapimageio.c`, 629–733 and
876–1138) allocate `width × height × 4` bytes from it before reading a pixel (668–670, 907–912). A
PNG of 65535 × 65535 compresses to a few kilobytes and would ask for 16 GiB. `msDrawShadeSymbol`
(`src/maprendering.c`, 853–872) sizes a polygon fill's tile from the image times the style's scale,
which `SIZE` sets, and from the style's `GAP` where it is positive, either one unbounded, and
`getTile` (234–236) allocates it. The maintainer has decided the limits; draft them in Q-9's list:

> … the size of a font file, which a symbol image's file shares (32 MiB, apart from that total),
> the pixels of a symbol image, read from its header before it is decoded, a GIF's from its
> logical screen (4096 × 4096), the pixels of the tile a polygon is filled with, whatever its size
> or gap asks (4096 × 4096), the images one render holds decoded, its fill tiles among them
> (256 MiB) …

A symbol image is read at render, as a font is, so it stands apart from the 16 MiB total of a
mapfile with its `INCLUDE`s and the files it names, and is read within the 32 MiB a font is read
within (`MAX_FONT_BYTES`), which the CLI enforces through the loader it hands the engine: it shares
the font's number rather than having a number of its own. Each limit is a diagnostic past it, with a
test per limit, as Q-9 already asks.

**The tiles count.** MapServer keeps at most six tiles per image (`MS_IMAGECACHESIZE`,
`src/mapsymbol.h`, 51; `addTileCache`, `src/maprendering.c`, 134–180), each up to 64 MiB at the tile
bound. The maintainer decided that the tiles a render holds count towards its 256 MiB of decoded
images, so no further number is needed; ticket 09 implements it.

The draft names every accepted ADR whose basis it moves, for the maintainer to confirm; the agent
judges none unaffected alone ([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7).

## Acceptance criteria

- [ ] Given the maintainer's decision on the wording, when it lands, then Q-9 names the pixels of
  a symbol image (4096 × 4096, a GIF's logical screen), a polygon fill's tile (4096 × 4096), and
  the decoded images of one render with its fill tiles (256 MiB), each with its number, and a
  symbol image's file sharing the font's 32 MiB, apart from the 16 MiB total.
- [ ] Given the landed wording, when the tickets of this spec that implement it are read, then
  each criterion that tests a limit states the number Q-9 states.

## Read first

- Q-9 in `docs/SPECIFICATION.md`; the spec `docs/plan/pixmap-symbols/spec.md` (Implementation
  decisions, the bullet *Already decided by the maintainer*);
  `docs/plan/revision-2026-10-31/07-spec-q9-markers-by-any-path.md` and
  `docs/plan/revision-2026-10-24/06-q9-feature-and-source-limits.md`, drafted changes of the same
  criterion, which may land with this one.

## Blocked by

- None (can start immediately)

## Mode

with the human: a change of the specification is the maintainer's
([`AGENTS.md` §3](../../../AGENTS.md#3-adr-rules), rule 7), and the resource limits are kept with
him by [ADR-0007](../../adr/0007-harness.md).
