# GIF pixmaps, read as MapServer's readGIF reads them

Observed: a pixmap symbol whose image is a GIF is not drawn; Forager decodes only PNG
(crates/encode/src/pixmap.rs). MapServer tells a GIF by its first bytes
(`msLoadMSRasterBufferFromFile`, src/mapimageio.c, 1159) and decodes it with `readGIF` (876–1138):

- the image is the logical screen, `SWidth × SHeight` (907–912); Q-9 bounds it at 4096 × 4096, so
  its size is held to that before anything is allocated for it;
- the screen is filled opaque with the background colour, an index past the colour table taken as
  0 (928–941); no table is an error (919–925). The table is chosen at 918, right after
  `DGifOpenFileName`, before any image descriptor is read, so the global table is always the one
  used and a frame's local table never is. That is inferred from giflib, not read at the giflib
  the noise build links;
- only the first frame is drawn, refused where its offset and size do not fit the screen
  (977–986); later frames are skipped (1067–1091). An interlaced frame is placed at its offset,
  rows in their four passes from `row + interlacedOffsets[count]` (989–995); a frame that is not
  interlaced is drawn from row 0 at its column, its `Top` ignored (1030–1032);
- the transparent index is set by a graphic control extension whose first sub-block has bit 0 of
  its byte 1 set, from its byte 4 (1105–1106). MapServer also takes it from every continuation
  sub-block of any extension whose byte 1 has bit 0 set (1120–1121), such as a looping GIF's
  `NETSCAPE2.0` sub-block `3, 1, lo, hi` or a comment's. A pixel is transparent where its index is
  the transparent one or lies past the colour table (1012–1023, 1048–1059).

Decided by the maintainer when this was a ticket: only the graphic control extension sets the
transparent index, as the GIF format defines it; docs/semantics/symbol.md states the deviation
from lines 1120–1121 when GIF pixmaps land. The decoder in view is image-rs's pure-Rust `gif`
crate, which no ADR has chosen yet: it enters Cargo.lock only under an ADR of ADR-0011's, its
licences checked by `cargo deny` and the bundle size of the bindings measured with it (Q-5). The
case renderers/point_pixmap_gif.map is listed as failing in the harness, its reason still "not
built yet: pixmap symbols" where GIF is what is missing.

Where: crates/encode/src/pixmap.rs, docs/semantics/symbol.md, crates/cli/tests/conformance/cases.txt
(`point_pixmap_gif`), Cargo.toml and deny.toml (ADR-0011), the convention on readers of untrusted
bytes in docs/CONVENTIONS.md.
