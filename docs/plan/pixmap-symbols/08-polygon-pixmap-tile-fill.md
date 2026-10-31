# Ticket: Fill a polygon with its pixmap symbol, tiled

## What to build

A polygon style whose symbol is a pixmap fills the polygon with the image repeated, as
`msDrawShadeSymbol` (`src/maprendering.c`, 733–898) fills it:

- no colour is needed for a pixmap (753); `OFFSET` moves the polygon first (770–783);
- the tile is `MS_NINT(width * scale)` by `MS_NINT(height * scale)` where the scale is not 1, the
  image's own size where it is, at least `MS_NINT(gap)` (or the gap) where the `GAP` is positive,
  and at least 1 (853–872); held to Q-9 as ticket 01 settles a tile;
- `getTile` (222–386) draws the image, scaled and turned as at a point, centred in a new
  transparent image of the tile's size (234–243, 263–269; `agg2CreateImage`, `src/mapagg.cpp`,
  864–867); no seamless mode for a pixmap (878–883);
- `agg2RenderPolygonTiled` (`src/mapagg.cpp`, 529–547) fills the polygon even-odd, without gamma,
  with the tile repeated from the image's origin (`image_accessor_wrap` with `wrap_mode_repeat`,
  `span_pattern_rgba` at offset 0, 0), blended as ADR-0034 blends.

`msDrawShape`'s clip buffer counts a pixmap style at the image's height (`msSymbolGetDefaultSize`,
`src/mapsymbol.c`, 74–79), where the engine counts 1 today (`docs/semantics/symbol.md`, *Not yet as
MapServer draws a hatch*); it counts the height, the image loaded before the clip.

## Acceptance criteria

- [ ] Given a polygon with a 32 × 32 pixmap at scale 1, at `SIZE 15 ANGLE 45`, and with `GAP 40`,
  when rendered, then the tile is 32 × 32, 15 × 15, and 40 × 40, stated in the test, its origin at
  the image's, and the engine's reference images hold the fills.
- [ ] Given a polygon with a hole, when filled with a pixmap, then the hole stays empty (even-odd).
- [ ] Given a tile whose size passes the bound ticket 01 sets for it, when rendered, then the style
  is not drawn with the limit's diagnostic, and a counting allocator bounds the peak; where ticket
  01 sets none, this criterion states the bound the maintainer chose instead.
- [ ] Given a class with a hatch style and a pixmap style, when its clip buffer is computed, then
  the pixmap counts at its image's height, stated in the test.
- [ ] Given `polygon-pixmap`, when the harness renders it, then it passes and the list says so.

## Read first

- [ADR-0015](../../adr/0015-agg-rust-rasterizer.md),
  [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md),
  [ADR-0032](../../adr/0032-hatch-cut-by-i-overlay.md) (the hatch's clip buffer it shares).
- `docs/semantics/symbol.md` (a hatch over a polygon), `docs/semantics/style-symbol.md` (a
  polygon's fill); the engine's polygon fill and hatch and their reference images, the golden path.
  Seams: the engine's render tests, the harness.

## Blocked by

- docs/plan/pixmap-symbols/01-q9-symbol-image-limit.md
- docs/plan/pixmap-symbols/03-pixmap-scaled-and-rotated.md

## Mode

with the human: the tile is an allocation `SIZE` alone sets, and its bound is a resource limit of
Q-9 that [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer.
