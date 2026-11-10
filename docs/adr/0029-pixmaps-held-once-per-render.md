# ADR-0029: A decoded pixmap is held once per render and per path, and shared by every item that draws it

- **Status:** 🟢 accepted
- **Date:** 2026-10-31
- **Deciders:** Markus Brigl
- **Applies to:** the image store of `crates/engine`, the type `PremultipliedImage` of `crates/render` and the display items that carry it, the pixmap decoders of `crates/encode`, and every crate that draws a pixmap or a fill tile

## Context

A `TYPE PIXMAP` symbol draws an image at each point of a layer, along a line at its `GAP`, and as
the repeated tile of a polygon's fill. One image may be placed at every feature of a layer, and
Q-9 lets a layer draw 1,000,000. Q-9 bounds a symbol image at 4096 × 4096 pixels, so one image
decoded takes up to 64 MiB, and the images one render holds decoded, its fill tiles among them, at
256 MiB. The engine keeps no display list: `draw_ops` (`crates/symbolize/src/draw.rs`) yields the
items of one feature and style, and the engine hands each to the renderer as it is produced, after
which it is dropped.

MapServer reads a pixmap when it is first drawn or measured (`msPreloadImageSymbol`,
`src/mapsymbol.c`, 952–972), takes the symbol's width and height from it (969–970), and keeps it
on the symbol until the symbol is freed (`msFreeSymbol`, 135–137). `readPNG` premultiplies each
pixel whose alpha is below 255 as `(c * a + 255) >> 8`, a transparent one to 0
(`src/mapimageio.c`, 714–729); a GIF pixel is opaque or transparent, so the same holds of
`readGIF`. The blender `render-agg` ports takes a premultiplied source
([ADR-0026](0026-agg-blender-and-resampler.md)).

Forager's engine keeps no state between renders. It loads font files this way: through the
resource loader each host hands it, once per render, when a label first needs one, the bytes
shared by `Rc` among the lines drawn in it ([ADR-0031](0031-label-glyphs.md)). A pixmap takes the
same path.

[ADR-0008](0008-ports-and-adapters-crates.md) lets `encode` depend on `render` and the `symbolize`
crate depend on `render` but not on `encode`, so the decoded image is a type of `render`.
`forager_render::RgbaImage` holds straight alpha: it is the render's output, what `encode::png`
writes, and what `encode::decode_png` returns for G-12's round trip and every reference image. The
pixmap decoder lives in `encode`, PNG under [ADR-0030](0030-png-encoder.md). The images reach the
browser among the files a page passes, as fonts do; the feature sources
[ADR-0024](0024-browser-bindings-bind-geojson-only.md) binds do not change. The bound sits beside
the per-render bound of the label cache ([ADR-0032](0032-label-cache.md)), which it leaves as it
is.

Today the store is `crates/engine/src/images.rs`, which holds the fill tiles too
(`crates/render/src/tile.rs`).

## Decision

We will hold each decoded pixmap once per render and per resource path, decoded on its first use
and premultiplied as MapServer's `readPNG` premultiplies, shared by reference by every display item
that draws it, counted with the fill tiles against Q-9's 256 MiB of decoded images, and released
when the render ends.

- **Store.** The engine keeps the decoded images of a render by resource path, each the image or
  the reason it has none. The first use loads the file through the resource loader and decodes it.
  Every later use in the render takes the same image. A failure must not be retried.
- **Type.** A decoded pixmap and a fill tile are a `forager_render::PremultipliedImage`, a type
  apart from `RgbaImage`, so that the compiler keeps premultiplied pixels from where straight ones
  are read. The pixmap decoder of `encode` returns it. `decode_png` and `png` stay straight
  (G-12).
- **Sharing.** A display item that draws a pixmap or a tile must hold a counted reference to the
  one image, never a copy of its pixels.
- **Bound.** The bytes of every decoded image and fill tile alive in the render count against
  256 MiB, whether the store, the tile cache, or anything else holds them. An image or tile that
  would pass the bound must be refused before it is decoded or built.
- **Lifetime.** The store belongs to one render. When the render ends, the images are freed.

**Out of scope:** how each format is decoded, and the GIF crate; the shape of the display items
that place an image or fill a polygon with a tile, left to the implementation under
[ADR-0025](0025-agg-rust-rasterizer.md); which fill tiles are built and how many are kept; the
values of Q-9's limits.

## Alternatives considered

- **A cache of decoded images across renders, in the host** — what MapServer's symbol does for a
  map object that outlives one draw, and it saves decoding for a server that renders one map
  often. It needs an eviction policy and a bound that spans renders, state shared between
  requests, and the same in every host; a render would no longer be a function of its inputs
  alone (Q-2).
- **A copy per placement** — each item owns its pixels. Since items are drawn as they are
  produced, the memory stays near one copy, but each placement copies up to 64 MiB: across
  1,000,000 features some 61 TiB of copying, which no bound per image prevents, against Q-9's
  bound on time.
- **Re-decoding per use** — no image is kept between placements, but each placement reads the
  file through the loader and decodes up to 64 MiB again, so the time grows with the placements
  times the pixels, against Q-9's bound on time. Sharing pays the read and the decode once.
- **Held per symbol, as MapServer holds it** — two symbols naming one file, or a style naming the
  file a symbol names, decode it twice; keyed by path, the store has the shape the fonts have.
- **Decoded with straight alpha and premultiplied at draw** — each placement premultiplies again,
  and the store would hold bytes MapServer never holds.
- **`RgbaImage` with a mark that it is premultiplied** — no new type, but the mark is a field the
  compiler does not check: the two dozen or so places that build an `RgbaImage` would each have
  to set it, and `encode::png` handed a premultiplied image would write it as straight and
  silently darken every pixel that is not opaque.

## Sources / Prior art

- MapServer at `rel-8-6-6`: `src/mapsymbol.c` (`msPreloadImageSymbol`, `msFreeSymbol`),
  `src/mapimageio.c` (`readPNG`, `readGIF`, `msLoadMSRasterBufferFromFile`), `src/mapagg.cpp`
  (`use_imagecache = 0`); read 2026-10-31.
- The engine's fonts, `crates/engine/src/fonts.rs`; the drawing of a style,
  `crates/symbolize/src/draw.rs`, and the engine's `crates/engine/src/lib.rs`; the pixmap spec.

## Consequences

- Positive: the memory of a render's pixmaps is bounded by the images it names, not by the
  features that place them, and Q-9's one number decides it; the cost of decoding is paid once
  per image.
- Positive: renders stay independent; no host keeps or evicts images.
- Positive: premultiplied and straight pixels cannot be mixed up without a conversion the code
  names.
- Negative / trade-offs: a host that renders one map many times decodes its images every render.
  `render` carries a second image type beside `RgbaImage`.
- Follow-ups: a cache across renders, if a server host measures decoding as a cost; a pixmap as a
  label's style, which would draw from the same store.

## Enforcement

Tests cite this ADR with `Verifies:`:

- `crates/engine/tests/pixmap_memory.rs`: a render placing one 512 × 512 pixmap at 10,000 points
  hands the renderer items that all refer to the same image, by pointer equality; it calls the
  loader once; the image is released once the render returns; and under a counting allocator the
  memory returns to its level before the render.
- `crates/engine/tests/images.rs`: an image is loaded and decoded once, however often it is asked
  for.
- `crates/engine/tests/image_memory.rs`: images whose decoded bytes come to more than 256 MiB,
  taken by the store in turn, have the one that would pass the bound refused before it is
  decoded.
- `crates/engine/tests/tile_memory.rs` and `crates/engine/tests/pixmaps.rs`: a polygon fill's tile
  counts against the same bound, and polygons of one pixmap style share one tile.
