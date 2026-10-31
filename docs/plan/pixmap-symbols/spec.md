# Spec: Pixmap symbols

## Problem

A mapfile that marks its points, lines, or polygons with an image draws a grey square in Forager
where MapServer draws the image. Seven cases of the conformance suite fail for it: `point_pixmap`,
`point_pixmap_gif`, `marker_pixmap` (text), `marker_anchorpoint`, `line_marker_pixmap`,
`polygon-pixmap`, and `poly_geomtransform_anchor`. A user's icon set, the most common way a
MapServer map marks a point of interest, is lost. And a label beside such an icon is not drawn at
all: the label cache cannot size the icon, so the engine leaves the label out with
`label-not-drawn`.

This spec makes five of the seven pass: `point_pixmap_gif`, `marker_pixmap`, `line_marker_pixmap`,
`polygon-pixmap`, and `poly_geomtransform_anchor`. Two stay `fails`, by the maintainer's decision:
`marker_anchorpoint`, with the reason "TrueType symbols", since 32 of its 56 points draw the
TrueType symbols `1` to `4` of its classes 1 to 4, which Forager does not draw yet; and
`point_pixmap`, with the reason "a symbol fetched by URL", since its fourth layer names
`http://localhost:8000/misc/data/gdalicon.png`, which MapServer at the pin, built with curl, fetches
and draws, and Forager never reads a URL (Q-8). The pixmap parts of both are built and decided by
their own tests.

## Solution

A user sees the image of a `TYPE PIXMAP` symbol drawn where MapServer draws it: at a point, scaled
by `SIZE`, turned by `ANGLE`, placed by its `ANCHORPOINT`; along a line with a `GAP`; as a tiled
fill of a polygon; and at a transformed geometry, `bbox` among the transforms. A style may name an
image file instead of a symbol, as MapServer allows. PNG and GIF images are read; any other format
is refused with a diagnostic, and so is an image larger than 4096 × 4096 pixels, before it is
decoded; an image that does not load leaves its style undrawn with a warning, and the rest of the
map draws. A label beside an image marker is placed against the image's size, as MapServer's label
cache places it. The browser takes the images from the page, by the path the mapfile names, as it
takes fonts.

## Existing behaviour

- **Frontend.** `loadSymbol` (`src/mapsymbol.c`, 151–313) is ported: `TYPE PIXMAP`, `IMAGE` resolved
  beside the file that holds the symbol (lines 239–250, `msBuildPath` against the symbol's path),
  `TRANSPARENT` (291–296), `ANCHORPOINT` (159–170), and the load error for a pixmap without an image
  (185–189). The style IR already carries a pixmap symbol's `IMAGE` as a resource path inside the
  base context, not read at load, and its `TRANSPARENT`; no field of the style document changes for
  a symbol of a symbol file. A style's `SYMBOL` naming no symbol is kept as a name with the warning
  `symbol-unknown`, pinned by the frontend's tests `an_unknown_symbol_is_a_warning` and
  `an_index_past_the_symbols_or_a_quoted_number_warns_and_a_negative_one_is_refused`, which ticket
  05 changes with a reason each. Pinned too by the load of every msautotest mapfile (G-1).
- **Engine.** A point style whose symbol is a pixmap draws a square of its size (`Mark::Square`);
  the size of a pixmap marker under a label is "not computed yet", so a point label whose class's
  first style is a pixmap is not drawn (`label-not-drawn`); a pixmap counts in a hatch's clip
  buffer at a default size of 1. Pinned by the engine's tests of symbols and labels; the square
  is pinned by no test of a pixmap and goes.
- **Resources.** The engine reads font files through the resource loader once per render, each
  bounded at 32 MiB (ADR-0028); the CLI hands it a loader bounded at that size, the browser
  bindings one over the files a page passes. Neither reads an image today. Pinned by the engine's
  tests of fonts and the bindings' test of a page's fonts.
- **Encoder.** `encode` encodes the map as PNG and decodes 8-bit RGBA PNG only (ADR-0017). The
  browser bundle does not link it.
- **`GEOMTRANSFORM bbox`** is not carried over: the frontend warns and the style draws the
  feature's own geometry. The ticket of the spec `geometry-styles` that stopped on it says why:
  the style IR has no variant for it.
- **Harness.** The seven cases are listed as `fails` with "not built yet: pixmap symbols" (or the
  size of a pixmap symbol, or `bbox`).

## Goals served

- G-6, for the four cases of the category geometry and symbols that pass; G-13 and G-10, for
  `marker_pixmap` of the category text, whose two labels MapServer discards.
- Q-4: the share of passing cases grows.
- Q-9: three new limits, on the pixels of a symbol image, on a polygon fill's tile, and on the
  decoded images of one render, a change of the specification drafted by the first ticket. Q-8:
  an image a style names by its file resolves inside the base context.
- Q-1, Q-2, G-9: the decoders are pure Rust, build for `wasm32-unknown-unknown`, and decode the
  same bytes on every target. Q-5: the decoders enter the browser bundle and are measured.
- Q-6: the new crate's licence passes `cargo deny`.

## User stories

1. As a user, I want a pixmap symbol drawn at each point of a point layer at its own size, so
   that my icons show as they do in MapServer.
2. As a user, I want `SIZE` and `ANGLE` to scale and turn the image as MapServer does, so that a
   larger or rotated icon looks the same.
3. As a user, I want `ANCHORPOINT` to put the chosen point of the image on the feature, so that a
   pin's tip marks the place.
4. As a user, I want a style that names an image file in `SYMBOL` to draw that image, so that a
   mapfile that never declares a symbol for its icon still renders.
5. As a user, I want GIF images read as MapServer reads them, with their transparent colour, so
   that my older icon sets keep working.
6. As a user, I want images placed along a line at the style's `GAP`, so that a route's markers
   repeat as in MapServer.
7. As a user, I want a polygon filled with its pixmap symbol repeated, so that a textured area
   looks as in MapServer.
8. As a user, I want a label beside an image marker placed against the image's size, so that it
   is placed, or left out, where MapServer places or leaves it.
9. As a user, I want `GEOMTRANSFORM bbox` to draw the style on the feature's bounding box, so that
   a mapfile that frames its features renders.
10. As a host operator, I want an image larger than the limit refused from its header before it is
    decoded, so that a hostile mapfile or image cannot exhaust memory.
11. As a user, I want an image that does not load, or of a format other than PNG or GIF, named in a
    diagnostic, so that I know which symbol is missing and why.
12. As a browser host author, I want to hand Forager the images a mapfile names as I hand it fonts,
    so that the page decides which files leave the server.

## Implementation decisions

What MapServer does, read at the pin, `7515cccf` (`rel-8-6-6`):

- **Reading an image.** `msPreloadImageSymbol` (`src/mapsymbol.c`, 952–973) reads the image
  through the renderer's `loadImageFromFile`, which the AGG renderer sets to
  `msLoadMSRasterBufferFromFile` (`src/mapagg.cpp`, 1444), and takes the symbol's width and height
  from the image (970–971). `msLoadMSRasterBufferFromFile` (`src/mapimageio.c`, 1140–1174) tells
  the format by the file's first bytes, not its name: the PNG signature (1157), `GIF` (1159),
  anything else "unsupported pixmap format" (1170). It is called when the symbol is first drawn or
  measured (`preloadSymbol`, `src/maprendering.c`, 101–132; `msGetMarkerSize`, `src/mapsymbol.c`,
  736–738), never at load.
- **PNG.** `readPNG` (`src/mapimageio.c`, 629–733) expands a palette, grey below 8 bits, and a
  `tRNS` chunk; strips 16 bits to 8; turns grey into RGB; adds an opaque alpha where there is none;
  and premultiplies each pixel whose alpha is below 255 as `(c * a + 255) >> 8`, a transparent one
  to 0 (714–729). What libpng does with a 16-bit or interlaced image under those calls is libpng's,
  read by ticket 02.
- **GIF.** `readGIF` (`src/mapimageio.c`, 876–1138) allocates the logical screen
  (`SWidth × SHeight`, 907–912), fills it opaque with the background colour, an index past the
  colour table taken as 0 (928–941); refuses a frame that does not fit the screen (977–986); draws
  the first frame only and skips the rest (970–1091); and draws a pixel transparent where its
  index is the transparent one or lies past the colour table (1012–1023, 1048–1059). Two quirks:
  only the interlaced branch adds the frame's `Top` to its rows (989–995), so a frame that is not
  interlaced is drawn from row 0 (1030–1032); and the transparent index is taken from byte 4 of a
  graphic control extension (1105–1106) and of every continuation sub-block of any extension whose
  byte 1 is odd (1120–1121), which for a looping GIF's `NETSCAPE2.0` sub-block of three bytes reads
  past it; Forager does not port that (below). The colour table is chosen (918) right
  after giflib's `DGifOpenFileName`, which reads only the screen descriptor, so the global table
  is used and a frame's local table never is; that is inferred from giflib, not read. A GIF pixel
  is opaque or transparent, so no premultiplication changes it.
- **`TRANSPARENT`** of a symbol is read, written, and copied (`loadSymbol` 291–296, `writeSymbol`
  334, `msCopySymbol` 1007; `src/mapscale.c` 455 sets it for the scale bar's symbol) and never drawn
  with: no drawing code under `src/` at the pin reads `transparentcolor`.
- **At a point.** `msDrawMarkerSymbolInternal` (`src/maprendering.c`, 906–1010) draws a pixmap
  without a colour (930–933), after `msAdjustMarkerPos` (945–950), through
  `agg2RenderPixmapSymbol` (980–983); the AGG renderer has no image cache (`use_imagecache = 0`,
  `src/mapagg.cpp`, 1417). `computeSymbolStyle` (37–79) sets no colour for a pixmap (47–48), so
  neither the style's colour nor its opacity reaches it, and its scale is the style's size, scaled
  and bounded by `MINSIZE` and `MAXSIZE` times the resolution factor, over the image's height
  (62–65, with `msSymbolGetDefaultSize`, `src/mapsymbol.c`, 74–79).
- **Drawing the image.** `agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 646–702): unturned and at
  scale 1, the image is blended onto the map at `MS_NINT(x - width / 2.)`,
  `MS_NINT(y - height / 2.)` with full cover (693–700); otherwise a square of half-side
  `MS_NINT(max(width, height) * scale * 1.415) / 2 + 1` about the point (679–687) is filled non-zero
  with AGG's `span_image_filter_rgba_bilinear_clip` over the inverse of the transform that centres,
  turns by `-rotation`, scales, and moves the image, with a transparent background (656–690).
- **The marker's size.** `msGetMarkerSize` (`src/mapsymbol.c`, 720–799) truncates the scaled size
  to an `int` (722, 750–753), bounds it by `MINSIZE` and `MAXSIZE` unscaled (754–755), and for a
  pixmap gives the image's own width and height at size 1, else the size as height and the width in
  the image's proportion (772–782). `msAdjustMarkerPos` (`src/maprendering.c`, 1013–1050) moves an
  anchor other than the centre by that size, turned with the symbol.
- **Under a label.** `msAddLabelGroup` and `msAddLabel` (`src/maplabel.c`, 343–376 and 590–626)
  hold the marker of a point layer's label as the box of `msGetMarkerSize` of the class's first
  style about the point; `computeMarkerBounds` (`src/mapdraw.c`, 3284 onwards) sizes a label's
  `labelpnt` marker the same way.
- **Along a line.** `msDrawLineSymbol` (`src/maprendering.c`, 600–731) places a pixmap with a
  `GAP` through `msImagePolylineMarkers` (679–686), turned with the line for a negative one; with
  no `GAP` it brushes the line with a tile (687–707). `msImagePolylineMarkers` (388–598) takes the
  marker's width and height as the image's scaled, at least 1 (400–401), anchors each marker
  (447–464), skips one outside the image grown by that size (467–474), and draws the pixmap
  (478–480, 559–561).
- **A polygon's fill.** `msDrawShadeSymbol` (`src/maprendering.c`, 733–898) needs no colour for a
  pixmap (753); its tile is the image's size times the scale, rounded with `MS_NINT` where the scale
  is not 1 and there is no positive `GAP` (858–859); with a positive `GAP` it is the larger of the
  scaled gap and the image's size times the scale, taken in `double` and truncated by the assignment
  to `int`, the gap rounded with `MS_NINT` first where the scale is not 1 (855–856, 863–864); at
  least 1 (867–870). `getTile` (222–386) builds a tile only where its cache holds none of the same
  symbol, size, scale, rotation, and colours (`searchTileCache`, 83–99; 230), a cache of at most six
  per image (`MS_IMAGECACHESIZE`, `src/mapsymbol.h`, 51; `addTileCache`, 134–180), and draws the
  image centred in a new transparent image of that size (234–243, 263–269; `agg2CreateImage`,
  `src/mapagg.cpp`, 864–867, clears an image without a background to no colour);
  `agg2RenderPolygonTiled` (`src/mapagg.cpp`, 529–547) fills the polygon even-odd, without gamma,
  with the tile repeated from the image's origin (`image_accessor_wrap`, `wrap_mode_repeat`,
  `span_pattern_rgba` at offset 0, 0).
- **An image named by a style.** `classResolveSymbolNames` (`src/mapfile.c`, 3723–3762), run over
  every class after the map and its symbol file are read (`resolveSymbolNames`, 7599–7609),
  resolves a style's or a label style's name through `msGetSymbolIndex` (408–426) with
  `try_addimage_if_notfound`; a name no symbol carries goes to `msAddImageSymbol`
  (`src/mapsymbol.c`, 386–462), which appends a symbol named by the file name: SVG by a `.svg`
  extension, a pixmap otherwise (408–416); a name starting `http` is fetched with curl into a
  temporary file, at most 1 MB, and fails the load where the fetch fails (418–449); any other is a
  path against the map's directory (450–458). No file is opened at load: a file that is missing
  fails at the first draw. `docs/semantics/style-symbol.md` says MapServer "fails the load where
  there is none"; that holds for a URL only, and the ticket that carries file names corrects it.
- **A pixmap that does not load** makes `preloadSymbol` fail, so `msDrawMarkerSymbolInternal`
  fails (923–925), the layer's draw fails, and `msDrawMap` (`src/mapdraw.c`, 508–519) returns no
  image at all.
- **`GEOMTRANSFORM bbox`.** `msStyleSetGeomTransform` (`src/mapgeomtransform.c`, 48–49) reads it;
  `msDrawTransformedShape` (161–188) shades the shape's bounds, each side held within the image
  grown by the padding `MS_MAX(width, size) + 3` truncated to an `int`, through
  `msDrawShadeSymbol`, which strokes it where the style has an outline colour and no colour
  (753–763).

What Forager does:

- **The engine reads the images.** The engine loads a pixmap's file through the host's resource
  loader the first time a render needs it, as it loads a font (ADR-0028), reads the image's size
  from its header, refuses it past the limit of Q-9 before decoding, decodes it once per render,
  and draws every placement from that one decoded image. The frontend reads no image, as today.
  How the decoded image is held and shared by the display list is the ADR candidate below.
- **Decoding.** PNG and GIF are told by their first bytes, as MapServer tells them, and decoded as
  `readPNG` and `readGIF` decode them, premultiplied as `readPNG` premultiplies; any other format,
  and an image that does not decode, is refused with a diagnostic naming the symbol and its path.
  The decoders live in `encode`, PNG with the `png` crate under ADR-0017, which already decides
  decoding PNG there; GIF with the pure-Rust `gif` crate of image-rs (0.14.2, MIT OR Apache-2.0, on
  crates.io; its one required dependency, `weezl`, is MIT OR Apache-2.0 too), chosen by an ADR
  (ADR-0012 leaves each crate to the ADR that first needs it). The decoded image is a type of
  `render`, so that the planned `symbolize` crate, synchronous, without I/O, and not allowed to
  depend on `encode` (ADR-0009), can draw it; the engine decodes and hands it on.
- **Tiles** of a polygon fill are built once per symbol, tile size, scale, and rotation in a render,
  never per feature, at most six held, as MapServer's cache holds them, and count towards the
  256 MiB of decoded images one render holds.
- **Drawing.** The display list gains an image placed at a point, blended unscaled at a whole pixel
  or resampled bilinearly under a transform, and a polygon filled with a repeated tile; the
  reference renderer draws them as `agg2RenderPixmapSymbol` and `agg2RenderPolygonTiled` do, with
  agg-rust's ports of the AGG span generators (ADR-0015), blended as ADR-0034 blends, and the
  image's square clipped as ADR-0039 clips.
- **A pixmap that does not load** — missing, of another format, broken, or past a limit — leaves
  its style undrawn and the layer warns, naming the symbol, its path, and why, and the rest of the
  map draws; never a substitute square. This deviates on purpose, by the maintainer's decision,
  from MapServer, which fails the layer and returns no map at all (`msDrawMap`, `src/mapdraw.c`,
  508–519); `docs/semantics/symbol.md` records the deviation where ticket 03 updates it. A label
  beside such a pixmap is left out, with the same warning (ticket 10), where MapServer's failing
  `msGetMarkerSize` fails the label's layer and the map (`src/maplabel.c`, 350–352, 606–608).
- **A GIF's transparent index** is set only by its graphic control extension (1105–1106), as the
  GIF format defines it; the continuation sub-blocks `readGIF` also reads it from (1120–1121), a
  looping GIF's `NETSCAPE2.0` among them, are ignored. This deviates from MapServer for such GIFs,
  which there draw one more colour transparent; ticket 06 states it in `docs/semantics/symbol.md`.
- **An image named by a style** is appended by the frontend to the map's symbols as a pixmap or SVG
  symbol in `classResolveSymbolNames`' order, its path resolved against the main mapfile's directory
  inside the base context (Q-8). The style document's shape does not change: the symbol is one more
  entry of the map's symbols. The `symbol-unknown` warning such a name raises at load today goes:
  whether its file exists is known at draw time, where a missing one warns. A name that starts with
  `http` is not fetched; the style draws nothing and the frontend warns, so that the load still has
  no error (G-1). `point_pixmap` stays `fails` with "a symbol fetched by URL". A symbol bound to a
  property whose value names a file, which `bindStyle` (`src/maputil.c`, 145–153) adds at draw time,
  stays as it is today.
- **Hosts.** The CLI reads images from its base context through the loader it hands the engine.
  The browser bindings take them among the files a page passes, by the path the mapfile names; the
  binding of feature sources, which ADR-0038 decides, does not change.
- **`GEOMTRANSFORM bbox`** becomes a value of the style IR's transform, a change of the style
  document format the maintainer decides (ADR-0007, ADR-0020, ADR-0036).
- **Resource limits** (Q-9, a specification change drafted by ticket 01), each a diagnostic past it,
  the style not drawn:
  - a symbol image is at most 4096 × 4096 pixels, read from its header before anything is
    allocated for it; a GIF's from its logical screen, which `readGIF` allocates
    (`src/mapimageio.c`, 907–912);
  - a polygon fill's tile is at most 4096 × 4096 pixels, whatever its size or gap asks;
  - the decoded images one render holds, its fill tiles among them, come to at most 256 MiB;
  - a pixmap file is read within 32 MiB, sharing the font file's number (`MAX_FONT_BYTES`) rather
    than having one of its own, apart from the 16 MiB total of a mapfile and the files it names; the
    CLI enforces it through the loader it hands the engine, the browser bindings do not, so the
    engine checks it too.
- **Already decided by the maintainer**, at the interrogation: `marker_anchorpoint` stays `fails`
  with "TrueType symbols", its pixmap anchor point built and tested by ticket 07; `point_pixmap`
  stays `fails` with "a symbol fetched by URL"; a pixmap that does not load leaves its style
  undrawn with a warning, and a label beside it is left out; a GIF's transparent index comes from
  its graphic control extension alone; Q-9 gains the limits above, the tiles counted in the
  256 MiB.
- **Order with other open tickets.**
  - `docs/plan/revision-2026-10-25/05-png-in-the-harness.md` settles what ADR-0017 covers of
    decoding with `png`; it goes before ticket 02, which decodes pixmaps in `encode` under ADR-0017,
    so that ticket 02 reads ADR-0017 as it then stands.
  - `docs/plan/revision-2026-10-31/01-symbolize-crate.md` moves the symbolization out of the engine
    into a synchronous crate without I/O that may not depend on `encode`. It goes before ticket 03,
    the first that draws a pixmap, so that the drawing is written once in its new home; ticket 03 is
    blocked by it. It also fixes where the decoded image type lives: in `render`.
  - `docs/plan/revision-2026-10-31b/12-symbol-resolved-once.md` reworks the engine's marker size and
    anchor shift, which tickets 07 and 08 extend for a pixmap; it goes before both, which it blocks.
  - Ticket 05 reaches the frontier once ticket 03 lands, and 06 and 09 once 04 does; 07 and 08 once
    04 and `docs/plan/revision-2026-10-31b/12-symbol-resolved-once.md` have. Open together, they
    share files: 06 to 09 the symbolization of a style and the engine's image store, 05 and 06 the
    symbols' handling. Worked in parallel, their branches collide; the frontier sensor does not
    report it yet (`docs/plan/revision-2026-10-31c/08-frontier-tickets-sharing-a-file.md` would make
    it), so they are claimed one at a time, or in pairs that share no file.

## Testing decisions

- The engine is tested with Forager's own reference images at the engine's render seam, with
  test images of Forager's own, each value the test states read from the C at the pin: the
  whole-pixel position of an unturned image, the square and transform of a turned one, the marker's
  size of `msGetMarkerSize`, the tile's size, the anchor's offset. The engine's tests of fonts are
  the golden path for loading through the resource loader.
- Decoding is tested in its crate: each expansion of `readPNG` and its premultiplication, each
  branch of `readGIF` named above (screen larger than the frame, background past the table,
  interlaced, transparent index, index past the table, later frames skipped), and a format other
  than PNG or GIF refused.
- Each limit is tested as Q-9 asks, a test per limit feeding an input past it and expecting the
  diagnostic: an image's header claiming 4097 × 4096, a GIF's screen past the limit, a tile asked
  past it, and decoded images past 256 MiB in one render; and as the conventions ask of a reader of
  untrusted bytes, each with few bytes behind it under a counting allocator that bounds the peak,
  as the shapefile adapter's memory test does.
- The conformance harness flips each case that passes to `passes` in its list; `marker_pixmap`
  also takes its `labels-hold` mark.
- The browser bindings are tested with an image passed as a page passes a font.

## Out of scope

- SVG symbols, and TrueType symbols, which `marker_anchorpoint` also needs; it stays `fails` with
  "TrueType symbols".
- Fetching an image by URL, which MapServer does with curl; `point_pixmap` stays `fails` with "a
  symbol fetched by URL".
- A pixmap brushed along a line without a `GAP` (the tile of `msDrawLineSymbol`, 687–707), and a
  pixmap as a label's style, unless a case of this spec needs it.
- A symbol bound to a property whose value names an image file.
- JPEG, WebP, and every format MapServer does not read.
- `TRANSPARENT` of a symbol, which MapServer does not draw with.
- The legend and the scale bar.

## ADR impact

- **Binding:** ADR-0007 (the resource limits, the resolution of resource paths, the style document
  format, `deny.toml` and the licences stay with the maintainer, and the cap of open branches);
  ADR-0009 (where decoding lives among the crates; the engine may depend on `encode`); ADR-0012 (the
  new crate); ADR-0015 and ADR-0034 (how the reference renderer resamples, tiles, and blends);
  ADR-0017 (PNG decoding in `encode`); ADR-0028 (the resource loader each host hands the engine);
  ADR-0035 (the label cache, for the marker under a label); ADR-0037 (`MS_NINT` and C's `int`
  conversions, as the image's position, the marker's size, and the tile's size use them); ADR-0039
  (the image's square and the tiled polygon clipped to the image); ADR-0014, ADR-0020, and ADR-0036
  (the `bbox` transform in the style document). ADR-0038 binds the browser bindings' feature
  sources, which this spec leaves as they are.
- **ADR candidate A, before ticket 06:** the GIF crate. ADR-0012 leaves the choice of each crate to
  the ADR that first needs it. Where pixmaps are decoded is not left to it: ticket 02 decodes PNG
  in `encode` under ADR-0017, and A puts the GIF decoder beside it, keeping ADR-0009's crates.
- **ADR candidate B, before ticket 02:** how a decoded pixmap is held. Once per render per path,
  premultiplied as MapServer premultiplies, shared by reference by every display item that draws
  it rather than copied per placement, and how the display list carries an image and a tile. It
  constrains the display list and the memory a render holds under Q-9; the maintainer decides
  whether it is an ADR or an implementation decision under ADR-0009 and ADR-0015.
- **A specification change:** the Q-9 limits of ticket 01.
