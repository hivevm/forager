# Ticket: Fill a polygon with its pixmap symbol, tiled

## What to build

A polygon style whose symbol is a pixmap fills the polygon with the image repeated, as
`msDrawShadeSymbol` (`src/maprendering.c`, 733–898) fills it:

- no colour is needed for a pixmap (753); `OFFSET` moves the polygon first (770–783);
- the tile's width and height are integers `pw` and `ph` (853–872), the gap `s.gap` being the
  `GAP` scaled as the size is (`computeSymbolStyle`, 66):
  - scale not 1, no positive gap: `MS_NINT(width * scale)` (858–859);
  - scale not 1, a positive gap: `MS_MAX(MS_NINT(s.gap), width * scale)`, the larger taken in
    `double` and then truncated by the assignment to `int`, so the image's term is truncated, not
    rounded (855–856);
  - scale 1, a positive gap: `MS_MAX(s.gap, width)`, truncated the same way (863–864);
  - scale 1, no positive gap: the image's own width (866);
  - then at least 1 (867–870);

  held to Q-9's 4096 × 4096, whatever the size or the gap asks, before anything is allocated for
  it: past it the style is not drawn, with the limit's diagnostic;
- `getTile` (222–386) draws the image, scaled and turned as at a point, centred in a new
  transparent image of the tile's size (234–243, 263–269; `agg2CreateImage`, `src/mapagg.cpp`,
  864–867); no seamless mode for a pixmap (878–883);
- `agg2RenderPolygonTiled` (`src/mapagg.cpp`, 529–547) fills the polygon even-odd, without gamma,
  with the tile repeated from the image's origin (`image_accessor_wrap` with `wrap_mode_repeat`,
  `span_pattern_rgba` at offset 0, 0), blended as ADR-0034 blends.

**Tiles are built once.** `getTile` looks a tile up in a cache of the image by its symbol, size,
scale, rotation, outline width, and colours (`searchTileCache`, 83–99; 230) and builds it only
where none matches (232–383), adding it to the cache, which holds at most six
(`MS_IMAGECACHESIZE`, `src/mapsymbol.h`, 51; `addTileCache`, 134–180, dropping the least recently
added). The cache changes no pixel, only the cost. The engine builds a tile once per symbol, tile
size, scale, and rotation in a render, never per feature, and holds at most six, as MapServer does,
a seventh dropping the one built longest ago. The tiles alive in the render, held or still
referenced, count towards the 256 MiB of decoded images one render holds
([ADR-0040](../../adr/0040-a-decoded-pixmap-is-held-once-per-render.md)), as the maintainer
decided (ticket 01): a tile that would pass it is not built, and its style is not drawn, with the
limit's diagnostic.

`msDrawShape`'s clip buffer counts a pixmap style at the image's height (`msSymbolGetDefaultSize`,
`src/mapsymbol.c`, 74–79), where the engine counts 1 today (`docs/semantics/symbol.md`, *Not yet as
MapServer draws a hatch*); it counts the height, the image loaded before the clip.

## Acceptance criteria

- [ ] Given a polygon with a 32 × 32 pixmap at scale 1, at `SIZE 15 ANGLE 45`, at `SIZE 15.5`
  without a gap, at `SIZE 15.5 GAP 10`, and at scale 1 with `GAP 40.7`, when rendered, then the
  tiles are 32 × 32, 15 × 15, 16 × 16 (`MS_NINT(15.5)`, half to even), 15 × 15 (15.5 truncated),
  and 40 × 40 (40.7 truncated), stated as literals in the test, their origin at the image's, and
  the engine's reference images hold the fills.
- [ ] Given a polygon with a hole, when filled with a pixmap, then the hole stays empty (even-odd).
- [ ] Given a 32 × 32 pixmap at a `SIZE` that asks a tile of 4097 × 4097 pixels, one at a `GAP`
  that asks it, and one at a `SIZE` that asks 4096 × 4096, when rendered, then the first two styles
  are not drawn, each with the limit's diagnostic before anything is allocated for its tile, and a
  counting allocator bounds the peak, while the third is drawn (Q-9).
- [ ] Given a thousand polygons drawn with one pixmap style, when rendered, then its tile is built
  once; given seven styles of distinct tiles, then at most six tiles are held at once.
- [ ] Given decoded images and tiles in one render that come to more than 256 MiB together, each
  within its own bound, when rendered, then the tile that would pass the 256 MiB is not built, its
  style is not drawn with the limit's diagnostic, the other styles are drawn, and a counting
  allocator bounds the peak (Q-9).
- [ ] Given a class with a hatch style and a pixmap style, when its clip buffer is computed, then
  the pixmap counts at its image's height, stated in the test.
- [ ] Given `polygon-pixmap`, when the harness renders it, then it passes and the list says so.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md),
  [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md),
  [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md) (the hatch's clip buffer it shares).
- `docs/semantics/symbol.md` (a hatch over a polygon), `docs/semantics/style-symbol.md` (a
  polygon's fill); ticket 01 on the tiles; the engine's polygon fill and hatch and their
  reference images, the golden path. Seams: the engine's render tests, the harness.

## Blocked by

- docs/plan/pixmap-symbols/01-q9-symbol-image-limit.md
- docs/plan/pixmap-symbols/04-pixmap-scaled-and-rotated.md

## Mode

with the human: this ticket is the code that enforces the tile's bound of Q-9, and
[ADR-0007](../../adr/0007-harness.md) keeps the resource limits with the maintainer however small
the ticket and whoever decided the number. Of the unattended conditions, tests would decide the
criteria and a revert undo them, but the third, touching nothing ADR-0007 keeps with the human,
cannot be answered yes. The fill is not split from its bound, because an unbounded tile would leave
Q-9 open between two pull requests.
