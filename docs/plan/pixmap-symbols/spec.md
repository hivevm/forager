# Spec: Pixmap symbols

## Problem

A mapfile that marks its points, lines, or polygons with an image draws a grey square in Forager
where MapServer draws the image. Seven cases of the conformance suite fail for it: `point_pixmap`,
`point_pixmap_gif`, `marker_pixmap` (text), `marker_anchorpoint`, `line_marker_pixmap`,
`polygon-pixmap`, and `poly_geomtransform_anchor`. A user's icon set, the most common way a
MapServer map marks a point of interest, is lost. And a label beside such an icon is not drawn at
all: the label cache cannot size the icon, so the engine leaves the label out with
`label-not-drawn`.

## Solution

A user sees the image of a `TYPE PIXMAP` symbol drawn where MapServer draws it: at a point, scaled
by `SIZE`, turned by `ANGLE`, placed by its `ANCHORPOINT`; along a line with a `GAP`; as a tiled
fill of a polygon; and at a transformed geometry, `bbox` among the transforms. A style may name an
image file instead of a symbol, as MapServer allows. PNG and GIF images are read; any other format
is refused with a diagnostic, and so is an image larger than 4096 × 4096 pixels, before it is
decoded. A label beside an image marker is placed against the image's size, as MapServer's label
cache places it. The browser takes the images from the page, by the path the mapfile names, as it
takes fonts.

## Existing behaviour

- **Frontend.** `loadSymbol` (`src/mapsymbol.c`, 151–313) is ported: `TYPE PIXMAP`, `IMAGE`
  resolved beside the file that holds the symbol (lines 239–250, `msBuildPath` against the
  symbol's path), `TRANSPARENT` (291–296), `ANCHORPOINT` (159–170), and the load error for a
  pixmap without an image (185–189). The style IR already carries a pixmap symbol's `IMAGE` as a
  resource path inside the base context, not read at load, and its `TRANSPARENT`; no field of the
  style document changes for a symbol of a symbol file. A style's `SYMBOL` naming no symbol is kept
  as a name with the warning `symbol-unknown`. Pinned by the frontend's tests of symbols and by the
  load of every msautotest mapfile (G-1).
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

- G-6, for the six cases of the category geometry and symbols; G-13 and G-10, for `marker_pixmap`
  of the category text, whose two labels MapServer discards.
- Q-4: the share of passing cases grows.
- Q-9: a new limit on the pixels of a symbol image, a change of the specification drafted by the
  first ticket. Q-8: an image a style names by its file resolves inside the base context.
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
  measured (`preloadSymbol`, `src/maprendering.c`, 101–132; `msGetMarkerSize`, 736–738), never at
  load.
- **PNG.** `readPNG` (`src/mapimageio.c`, 629–733) expands a palette, grey below 8 bits, and a
  `tRNS` chunk; strips 16 bits to 8; turns grey into RGB; adds an opaque alpha where there is none;
  and premultiplies each pixel whose alpha is below 255 as `(c * a + 255) >> 8`, a transparent one
  to 0 (714–729). What `png_set_strip_16` and `png_read_image` do with an interlaced or 16-bit image
  is libpng's, and the ticket that decodes PNG reads it at the version the noise build links.
- **GIF.** `readGIF` (`src/mapimageio.c`, 876–1138) allocates the logical screen
  (`SWidth × SHeight`, 907–912), fills it opaque with the background colour, an index past the
  colour table taken as 0 (928–941); refuses a frame that does not fit the screen (977–986); draws
  the first frame only, interlaced or not, and skips the rest (970–1088); takes the transparent
  index of a graphic control extension (1102–1124); and draws a pixel transparent where its index
  is the transparent one or lies past the colour table (1012–1023, 1048–1059). The colour table
  is chosen (918) before any image descriptor is read; whether a frame's local table can win
  there is giflib's, not read for this spec. A GIF pixel is opaque or transparent, so no
  premultiplication changes it.
- **`TRANSPARENT`** of a symbol is read and written (`loadSymbol` 291–296, `writeSymbol` 334) and
  never used in drawing: no file under `src/` at the pin reads `transparentcolor` otherwise.
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
  is not 1, at least the `GAP` where there is one, at least 1 (853–872); `getTile` (222–386) draws
  the image centred in a new transparent image of that size (234–243, 263–269; `agg2CreateImage`,
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
  PNG with the `png` crate already in the workspace (ADR-0017); GIF with the pure-Rust `gif` crate
  of image-rs (0.14.2, MIT OR Apache-2.0, on crates.io; its one required dependency, `weezl`, is MIT
  OR Apache-2.0 too), chosen by an ADR (ADR-0012 leaves each crate to the ADR that first needs it).
- **Drawing.** The display list gains an image placed at a point, blended unscaled at a whole pixel
  or resampled bilinearly under a transform, and a polygon filled with a repeated tile; the
  reference renderer draws them as `agg2RenderPixmapSymbol` and `agg2RenderPolygonTiled` do, with
  agg-rust's ports of the AGG span generators (ADR-0015), blended as ADR-0034 blends, and the
  image's square clipped as ADR-0039 clips.
- **A pixmap that does not load** leaves its style undrawn and the layer warns, naming the symbol,
  its path, and why, where MapServer fails the whole map; a reader that fails closes, never a
  substitute square. This deviation is asked of the maintainer below.
- **An image named by a style** is appended by the frontend to the map's symbols as a pixmap or SVG
  symbol in `classResolveSymbolNames`' order, its path resolved against the main mapfile's
  directory inside the base context (Q-8). The style document's shape does not change: the symbol
  is one more entry of the map's symbols. A name that starts with `http` is not fetched; the
  style draws nothing and the frontend warns, so that the load still has no error (G-1). A symbol
  bound to a property whose value names a file, which `bindStyle` (`src/maputil.c`, 145–153) adds
  at draw time, stays as it is today.
- **Hosts.** The CLI reads images from its base context through the loader it hands the engine.
  The browser bindings take them among the files a page passes, by the path the mapfile names; the
  binding of feature sources, which ADR-0038 decides, does not change.
- **`GEOMTRANSFORM bbox`** becomes a value of the style IR's transform, a change of the style
  document format the maintainer decides (ADR-0007, ADR-0020, ADR-0036).
- **Resource limit.** A symbol image may be at most 4096 × 4096 pixels, read from its header before
  anything is allocated for it; past that it is not drawn, with a diagnostic (Q-9, a specification
  change drafted by ticket 01, with the further bounds it asks about).

## Testing decisions

- The engine is tested with Forager's own reference images at `crates/engine`'s render seam, with
  test images of Forager's own, each value the test states read from the C at the pin: the
  whole-pixel position of an unturned image, the square and transform of a turned one, the marker's
  size of `msGetMarkerSize`, the tile's size, the anchor's offset. The engine's tests of fonts are
  the golden path for loading through the resource loader.
- Decoding is tested in its crate: each expansion of `readPNG` and its premultiplication, each
  branch of `readGIF` named above (screen larger than the frame, background past the table,
  interlaced, transparent index, index past the table, later frames skipped), and a format other
  than PNG or GIF refused.
- The limit is tested as Q-9 asks, an input past it expecting the diagnostic, and as the
  conventions ask of a reader of untrusted bytes: a header claiming 4097 × 4097, or 65535 × 65535,
  with few bytes behind it, under a counting allocator that bounds the peak, as the shapefile
  adapter's memory test does.
- The conformance harness flips each case that passes to `passes` in its list; `marker_pixmap`
  also takes its `labels-hold` mark.
- The browser bindings are tested with an image passed as a page passes a font.

## Out of scope

- SVG symbols, and TrueType symbols, which `marker_anchorpoint` also needs (see the open
  questions).
- Fetching an image by URL, which MapServer does with curl.
- A pixmap brushed along a line without a `GAP` (the tile of `msDrawLineSymbol`, 687–707), and a
  pixmap as a label's style, unless a case of this spec needs it.
- A symbol bound to a property whose value names an image file.
- JPEG, WebP, and every format MapServer does not read.
- `TRANSPARENT` of a symbol, which MapServer does not draw with.
- The legend and the scale bar.

## Open questions for the maintainer

1. **`marker_anchorpoint` needs TrueType symbols.** Its classes 1 to 4 draw the TrueType symbols
   `1` to `4`, and 32 of the 56 points of `rotpoints.dbf` are of those classes; Forager draws no
   TrueType symbol yet (`point_truetype` fails for it). The case cannot pass under this spec. Keep
   it `fails` with the reason "TrueType symbols" once its pixmap half is drawn, or bring TrueType
   symbols into scope as a spec of their own?
2. **`point_pixmap` fetches an image by URL.** Its fourth layer names
   `http://localhost:8000/misc/data/gdalicon.png`; MapServer at the pin is built with curl
   (`scripts/build-mapserver.sh`) and the reference image shows the icon drawn. Forager reads no URL
   (Q-8), so the case cannot pass. Keep it `fails` with the reason "a symbol fetched by URL", or
   decide a new reason for leaving a case out of scope?
3. **A pixmap that does not load** fails the whole map in MapServer (`msDrawMap`, 508–519). Is
   leaving the style undrawn with a warning, as for a font, the deviation you want?
4. **Q-9 beyond the pixels of one image.** Ticket 01 drafts the 4096 × 4096 limit and asks: does it
   also bound a polygon fill's tile, whose size `SIZE` alone sets at the pin (853–872)? Is a pixmap
   file bounded in bytes, and at what? Is there a bound on the decoded images one render holds,
   each up to 64 MiB at the limit? For a GIF, is the limit read from its logical screen, which
   `readGIF` allocates?

## ADR impact

- **Binding:** ADR-0007 (the resource limits, the resolution of resource paths, the style document
  format, `deny.toml` and the licences stay with the maintainer, and the cap of open branches);
  ADR-0009 (where decoding lives among the crates; the engine may depend on `encode`); ADR-0012
  (the new crate); ADR-0015 and ADR-0034 (how the reference renderer resamples, tiles, and blends);
  ADR-0017 (PNG decoding in `encode`); ADR-0028 (the resource loader each host hands the engine);
  ADR-0035 (the label cache, for the marker under a label); ADR-0039 (the image's square and the
  tiled polygon clipped to the image); ADR-0014, ADR-0020, and ADR-0036 (the `bbox` transform in
  the style document). ADR-0038 binds the browser bindings' feature sources, which this spec leaves
  as they are.
- **ADR candidate A, before ticket 05:** the GIF crate and where pixmaps are decoded. ADR-0012
  leaves the choice of each crate to the ADR that first needs it, and ADR-0017 decides PNG only, in
  `encode`. Decoding both in `encode` keeps ADR-0009's crates; a crate of its own changes ADR-0009's
  map.
- **ADR candidate B, before ticket 02:** how a decoded pixmap is held. Once per render per path,
  premultiplied as MapServer premultiplies, shared by reference by every display item that draws
  it rather than copied per placement, and how the display list carries an image and a tile. It
  constrains the display list and the memory a render holds under Q-9; the maintainer decides
  whether it is an ADR or an implementation decision under ADR-0009 and ADR-0015.
- **A specification change:** the Q-9 limit of ticket 01.
