# Ticket: Decode PNG pixmaps within Q-9's limits, once per render

## What to build

The reading half of the tracer bullet: the engine gets a pixmap's decoded image, or the reason it
has none, without drawing it yet (ticket 03 draws it).

- **Decoder, in `encode`, under [ADR-0017](../../adr/0017-png-crate.md).** A PNG is told by its
  signature, as `msLoadMSRasterBufferFromFile` (`src/mapimageio.c`, 1140–1174) tells it (1157);
  any other first bytes are refused as an unsupported format (1170), a GIF too, until ticket 06. Its
  width and height are read from the header and held to 4096 × 4096 before anything is allocated
  for them. It is decoded as `readPNG` (629–733) decodes it: a palette, grey below 8 bits, and a
  `tRNS` chunk expanded, 16 bits stripped to 8, grey turned to RGB, an opaque alpha added where
  there is none, and each pixel whose alpha is below 255 premultiplied as `(c * a + 255) >> 8`, a
  transparent one to 0 (714–729). What libpng's `png_set_strip_16` keeps of a 16-bit sample and how
  `png_read_image` handles an interlaced image without `png_set_interlace_handling` is libpng's:
  read both at the libpng the noise build links (`scripts/build-mapserver.sh`), or ask, before
  stating the expected bytes. The decoded image is a type of `render`, which `encode`, the engine,
  and a later `symbolize` crate may all depend on (ADR-0009), premultiplied and marked so.
- **Image store, in the engine.** The engine loads a pixmap's file through the resource loader the
  first time a render needs it, as it loads a font (ADR-0028), and decodes it once per render per
  path; every later use takes the same image. A file over 32 MiB, the bound of the load path
  fonts share (`MAX_FONT_BYTES`), is refused by the engine itself, since the browser bindings'
  loader does not bound it. The images one render holds decoded come to at most 256 MiB; an image
  that would pass it is not decoded. Each failure — not loaded, too large a file, not PNG, broken,
  past a limit — is a reason naming the path, which ticket 03 turns into the style's warning.

## Acceptance criteria

- [ ] Given PNGs of palette, palette with `tRNS`, grey of 1, 2, 4, and 8 bits, grey with alpha,
  RGB, RGBA, 16-bit RGBA, and an interlaced RGBA, when each is decoded, then its pixels are those
  `readPNG` yields under the libpng read above, each premultiplied as `(c * a + 255) >> 8` where
  its alpha is below 255 and 0 where it is 0, the expected bytes stated in the test.
- [ ] Given a PNG whose header claims 4097 × 4096 pixels, and one claiming 4096 × 4097, each with a
  few bytes behind it, when decoded, then each is refused with the limit's reason, and a counting
  allocator bounds the peak allocation far below the claimed size (Q-9).
- [ ] Given images, each within 4096 × 4096, that come to more than 256 MiB decoded, when the store
  takes them in turn, then the one that would pass the bound is refused with the limit's reason
  before it is decoded, the others are held, and a counting allocator bounds the peak (Q-9).
- [ ] Given a file of 32 MiB and one byte, when the store loads it, then it is refused with the
  file bound's reason.
- [ ] Given one path asked for a thousand times in a render, when the store serves it, then the
  loader is called and the image decoded once.
- [ ] Given a file that is missing, one that is no PNG, a GIF, and a truncated PNG, when the store
  loads each, then each is refused with a reason naming the path.

## Read first

- [ADR-0007](../../adr/0007-harness.md), [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0017](../../adr/0017-png-crate.md), [ADR-0028](../../adr/0028-text-stack.md) (Fonts);
  ADR candidate B of the spec (how a decoded image is held), proposed through `propose-adr` first
  where the maintainer asks for it; ADR candidate A, which decides only the GIF crate, since this
  ticket decodes PNG in `encode` under ADR-0017.
- The spec `docs/plan/pixmap-symbols/spec.md`; the conventions on readers of untrusted bytes and on
  ported behaviour. Golden path: the engine's fonts and `crates/engine/tests/fonts.rs`; the memory
  test of `crates/adapter-shapefile/tests/memory.rs`. Seams: `encode`'s unit tests, the engine's
  tests of its store.

## Blocked by

- docs/plan/pixmap-symbols/01-q9-symbol-image-limit.md
- docs/plan/revision-2026-10-25/05-png-in-the-harness.md

## Mode

with the human: it implements three resource limits of Q-9 and takes the decision of ADR candidate
B, both of which [ADR-0007](../../adr/0007-harness.md) keeps with the maintainer.
