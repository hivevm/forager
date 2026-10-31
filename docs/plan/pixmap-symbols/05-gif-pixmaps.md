# Ticket: Read GIF pixmaps as MapServer reads them

## What to build

A pixmap symbol whose image is a GIF, told by its first bytes (`msLoadMSRasterBufferFromFile`,
`src/mapimageio.c`, 1159), drawn as the PNG of ticket 02, decoded as `readGIF` (876–1138) decodes
it, with the pure-Rust `gif` crate of image-rs, once ADR candidate A of the spec has chosen it:

- the image is the logical screen, `SWidth × SHeight` (907–912), its size read from the header and
  held to Q-9's 4096 × 4096 before anything is allocated for it, the logical screen being what
  `readGIF` allocates;
- the screen is filled opaque with the background colour, an index past the colour table taken as
  0 (928–941); the colour table is chosen at line 918, the frame's local one where giflib holds one
  by then, else the global one, before any image descriptor is read, so whether a local table is
  ever used is giflib's to say: read `DGifOpenFileName` at the giflib the noise build links, or ask;
  no table is an error (919–925);
- only the first frame is drawn, at its offset, refused where it does not fit the screen
  (977–986); interlaced rows are placed in their four passes (989–1028); later frames are skipped;
- the transparent index is that of a graphic control extension (1102–1124), and a pixel is
  transparent where its index is that one or lies past the colour table (1012–1023, 1048–1059),
  opaque in its colour otherwise.

`gif` and its dependencies enter `Cargo.lock` under ADR-0012, their licences checked by
`cargo deny`; the bundle size of the bindings is measured with them (Q-5).

## Acceptance criteria

- [ ] Given GIFs of Forager's own for each branch above — a frame smaller than the screen, a
  background index past the table, an interlaced frame, a transparent index, an index past the
  table, two frames, a local and a global table, no table, a frame past the screen — when each is
  decoded, then its pixels, or its refusal, are those the C yields, the bytes stated in the test.
- [ ] Given a GIF whose logical screen claims 4097 × 4096 pixels, and one whose screen is within
  the limit but whose first frame claims more, when the render needs it, then the first is refused
  with the limit's diagnostic before anything is allocated for it, the second as a frame past the
  screen, each style not drawn, and a counting allocator bounds the peak (Q-9).
- [ ] Given `point_pixmap_gif`, when the harness renders it, then it passes and the list says so.
- [ ] Given the workspace, when `cargo deny check` and the build for `wasm32-unknown-unknown` run,
  then both are green.

## Read first

- [ADR-0007](../../adr/0007-harness.md), [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0012](../../adr/0012-crate-supply-chain.md), [ADR-0017](../../adr/0017-png-crate.md); the
  ADR that candidate A of the spec becomes.
- Ticket 02's decoder and its tests, the golden path; the convention on readers of untrusted
  bytes. Seams: the decoder's unit tests, the engine's render tests, the harness.

## Blocked by

- docs/plan/pixmap-symbols/03-pixmap-scaled-and-rotated.md

## Mode

with the human: it adds a dependency, which an ADR the maintainer decides must choose first
(ADR-0012), touches the licences `cargo deny` checks, and implements a resource limit, all of which
[ADR-0007](../../adr/0007-harness.md) keeps with him.
