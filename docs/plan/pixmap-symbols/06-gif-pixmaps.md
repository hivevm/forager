# Ticket: Read GIF pixmaps as MapServer reads them

## What to build

A pixmap symbol whose image is a GIF, told by its first bytes (`msLoadMSRasterBufferFromFile`,
`src/mapimageio.c`, 1159), drawn as the PNG of tickets 02 and 03, decoded as `readGIF` (876–1138)
decodes it, with the pure-Rust `gif` crate of image-rs, once ADR candidate A of the spec has chosen
it:

- the image is the logical screen, `SWidth × SHeight` (907–912), its size read from the header and
  held to Q-9's 4096 × 4096 before anything is allocated for it, the logical screen being what
  `readGIF` allocates;
- the screen is filled opaque with the background colour, an index past the colour table taken as
  0 (928–941); no table is an error (919–925). The table is chosen at 918, right after
  `DGifOpenFileName`, before any image descriptor is read; giflib's `DGifOpenFileName` reads only
  the header and the screen descriptor, so the global table is always the one used and a frame's
  local table never is. That is inferred from giflib, not read at the giflib the noise build links;
  the ticket reads it there before porting it;
- only the first frame is drawn, refused where its offset and size do not fit the screen
  (977–986); later frames are skipped (1067–1091). An interlaced frame is placed at its offset,
  rows in their four passes from `row + interlacedOffsets[count]` (989–995); a frame that is not
  interlaced is drawn from row 0 at its column, its `Top` ignored, since that loop starts at `i = 0`
  and adds no `row` (1030–1032);
- the transparent index is set by a graphic control extension whose first sub-block has bit 0 of
  its byte 1 set, from its byte 4 (1105–1106), and also by every continuation sub-block of any
  extension whose byte 1 has bit 0 set, from its byte 4 (1120–1121). A pixel is transparent where
  its index is the transparent one or lies past the colour table (1012–1023, 1048–1059), opaque in
  its colour otherwise.

**Question for the maintainer: the continuation sub-block.** A looping GIF carries the
`NETSCAPE2.0` application extension, whose continuation sub-block is `3, 1, lo, hi`: byte 1 is 1,
so lines 1120–1121 take the transparent index from byte 4, one past the sub-block's three bytes.
What that byte holds is giflib's buffer: inferred, not read, it is still `S` (83) from the
`NETSCAPE2.0` identifier read before it, so such a GIF would draw index 83 transparent. A
continuation sub-block of four bytes or more whose byte 1 is odd, such as a comment's, sets the
index from a byte it holds. Port both as MapServer behaves, the over-read included as giflib's
buffer leaves it; port the in-bounds case and ignore the over-read; or refuse a GIF that would
over-read? The ticket's criteria take the answer before it starts.

`gif` and its dependencies enter `Cargo.lock` under ADR-0012, their licences checked by
`cargo deny`; the bundle size of the bindings is measured with them (Q-5).

## Acceptance criteria

- [ ] Given GIFs of Forager's own for each branch above — a frame smaller than the screen, a
  background index past the table, an interlaced frame, a transparent index, an index past the
  table, two frames, a local and a global table, no table, a frame past the screen, a frame not
  interlaced with a `Top` above 0 (drawn from row 0), and an interlaced one with a `Top` above 0
  (drawn at it) — when each is decoded, then its pixels, or its refusal, are those the C yields,
  the bytes stated in the test.
- [ ] Given a GIF with a `NETSCAPE2.0` loop extension, and one with a comment whose continuation
  sub-block's byte 1 is odd, when decoded, then the transparent index is what the maintainer's
  answer to the question above says, or the GIF is refused as it says, stated in the test.
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
- Ticket 02's decoder and its tests, the golden path; the maintainer's answer to the question
  above; the convention on readers of untrusted
  bytes. Seams: the decoder's unit tests, the engine's render tests, the harness.

## Blocked by

- docs/plan/pixmap-symbols/04-pixmap-scaled-and-rotated.md

## Mode

with the human: it adds a dependency, which an ADR the maintainer decides must choose first
(ADR-0012), touches the licences `cargo deny` checks, and implements a resource limit, all of which
[ADR-0007](../../adr/0007-harness.md) keeps with him.
