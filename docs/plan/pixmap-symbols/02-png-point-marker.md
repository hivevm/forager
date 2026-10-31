# Ticket: Draw a PNG pixmap symbol at a point (tracer bullet)

## What to build

The first path through every layer: a point layer's style whose symbol, from the map or its symbol
file, is a `TYPE PIXMAP` with a PNG `IMAGE`, drawn unturned at its own size, as MapServer draws it,
from the CLI and from the browser bindings.

- **Frontend.** Nothing changes: the style IR already carries the symbol's `IMAGE` as a resource
  path resolved beside the file that holds it (`loadSymbol`, `src/mapsymbol.c`, 239–250).
- **Engine.** The point mark of a pixmap symbol is the image, not the square. The engine loads the
  image through the resource loader the first time the render needs it, as it loads a font
  (ADR-0028), reads its width and height from the PNG header, refuses it past Q-9's limit before
  anything is allocated for it, decodes it once per render as `readPNG` (`src/mapimageio.c`,
  629–733) decodes it, premultiplied as it premultiplies (714–729), and hands the display list an
  image placed at the point. A file whose first bytes are no PNG signature is refused as
  "unsupported pixmap format" (`msLoadMSRasterBufferFromFile`, 1140–1174); a GIF is refused for now,
  until ticket 05. A pixmap that does not load, decode, or keep to a limit leaves its style
  undrawn and the layer warns with a diagnostic naming the symbol, its path, and why, and the rest
  of the map draws: a deliberate deviation, decided by the maintainer, from MapServer, whose
  `msDrawMap` (`src/mapdraw.c`, 508–519) returns no map when a layer fails to draw.
  `docs/semantics/symbol.md` says so, beside what the engine now draws of a pixmap at a point.
- **Memory.** The images one render holds decoded come to at most 256 MiB (Q-9); an image that
  would pass it is not decoded, and its style is refused as above.
- **Renderer.** Unturned and at scale 1, `agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 693–700)
  blends the image with full cover at `MS_NINT(x - width / 2.)`, `MS_NINT(y - height / 2.)`; the
  reference renderer does the same, blended as ADR-0034 blends a premultiplied source. A style's
  colour and opacity do not reach the image (`computeSymbolStyle`, `src/maprendering.c`, 47–48).
- **Hosts.** The CLI reads the image from its base context; the browser bindings take it among
  the files a page passes, by the path the mapfile names. The bundle size of the bindings is
  measured with the PNG decoder in it (Q-5).
- The glossary gains the term the code names the decoded image by, if it has none.

A turned or scaled image is ticket 03; this one draws a pixmap whose scale is not 1, or whose
angle is not 0, as the square it draws today.

## Acceptance criteria

- [ ] Given a point layer with a 32 × 32 RGBA PNG pixmap symbol at a point whose pixel is
  (50.5, 41.5), when it is rendered, then the image's top-left pixel lies at (34, 26), `MS_NINT`
  rounding 34.5 and 25.5 half to even, its pixels blended premultiplied over the map, and the
  engine's reference image holds it; the positions are literals in the test, not the engine's.
- [ ] Given PNGs of palette, palette with `tRNS`, grey of 1, 2, 4, and 8 bits, grey with alpha,
  RGB, RGBA, and 16-bit RGBA, when each is decoded, then its pixels are those `readPNG` yields,
  each premultiplied as `(c * a + 255) >> 8` where its alpha is below 255 and 0 where it is 0, the
  expected bytes stated in the test.
- [ ] Given a PNG whose header claims 4097 × 4096 pixels, and one claiming 4096 × 4097, each with
  a few bytes behind it, when the render needs it, then the style is not drawn, the layer warns
  with the limit's diagnostic, and a counting allocator bounds the render's peak allocation far
  below the claimed size (Q-9).
- [ ] Given a map whose pixmaps, each within 4096 × 4096, come to more than 256 MiB decoded, when
  rendered, then the image that would pass the bound is not decoded, its style is not drawn with
  the limit's diagnostic, the styles of the images within it are drawn, and a counting allocator
  bounds the peak (Q-9).
- [ ] Given a pixmap whose file is missing, one that is no PNG, and one that is a truncated PNG,
  when rendered, then each style is not drawn, the warning names the symbol and the path, and the
  map is drawn with every other layer and style.
- [ ] Given one image placed at a thousand points, when rendered, then it is loaded and decoded
  once.
- [ ] Given the browser bindings and a page that passes the image beside the mapfile, when
  rendered, then the image is drawn as the CLI draws it, byte for byte.

## Read first

- [ADR-0007](../../adr/0007-harness.md), [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0015](../../adr/0015-agg-rust-rasterizer.md), [ADR-0017](../../adr/0017-png-crate.md),
  [ADR-0028](../../adr/0028-text-stack.md) (Fonts),
  [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md); ADR candidate B of
  the spec, proposed through `propose-adr` first where the maintainer asks for it.
- The spec `docs/plan/pixmap-symbols/spec.md`; `docs/semantics/symbol.md`; the conventions on
  readers of untrusted bytes, on what a render holds per feature, and on ported behaviour.
- Golden path: the engine's fonts and their test, `crates/engine/tests/fonts.rs`; the memory test
  of `crates/adapter-shapefile/tests/memory.rs`; the bindings' test of a page's fonts. Seams: the
  engine's render tests with reference images, `encode`'s unit tests.

## Blocked by

- docs/plan/pixmap-symbols/01-q9-symbol-image-limit.md

## Mode

with the human: it implements a resource limit of Q-9 and takes the decision of ADR candidate B,
how a decoded image is held and carried by the display list, both of which
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer.
