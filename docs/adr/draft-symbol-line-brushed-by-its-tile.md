# ADR-draft-symbol-line-brushed-by-its-tile: A symbol along a line without a GAP brushes the line with its tile, through agg-rust's outline-image rasterizer and a port of AGG 2.4's pattern filter

- **Status:** 🟡 proposed
- **Date:** 2026-11-16
- **Deciders:** Markus Brigl
- **Applies to:** the drawing of a line style whose symbol is a vector, ellipse, pixmap, or TrueType symbol and whose `GAP` is 0 in `crates/symbolize`, the display item of a line drawn with a tile in `crates/render`, its rasterizing, its pattern filter, and its clip in `crates/render-agg`

## Context

A line style whose symbol is a vector, an ellipse, a pixmap, or a TrueType symbol, and whose `GAP`
is 0, is drawn by Forager as a plain line in the style's `COLOR`. A pixmap there also warns
`pixmap-not-drawn`. MapServer draws something else (`src/maprendering.c` at rel-8-6-6, 687-707):

- `msDrawLineSymbol` asks `getTile` for a tile of `MS_NINT(sizex × scale)` by
  `MS_NINT(sizey × scale)` pixels, or of the extent truncated to `int` at a scale of 1, at least
  1 each. It asks for no seamless mode. `getTile` draws the symbol once at its centre, each kind
  as at a point, and keeps the tile in the image's tile cache, which `msDrawShadeSymbol` shares.
  `searchTileCache` keys a tile by symbol, size, scale, rotation, outline width, and colours, not
  by seamless mode. So a vector symbol's seamless tile, built for a polygon without a `GAP`,
  serves a line of the same key, and the other way round. Forager's tile cache keeps the same key.
- `agg2RenderLineTiled` (`src/mapagg.cpp`, 250-273) brushes the line with that tile. It uses
  AGG's `line_image_pattern` with `pattern_filter_bilinear_rgba8`, `renderer_outline_image`,
  and `rasterizer_outline_aa` with `line_coord_sat`, and sets no clip box. The line's parts go
  in as `move_to` and `line_to` through `line_adaptor`.
- The image's pixel format is `blender_rgba_pre`, premultiplied, so the pattern is sampled from
  premultiplied pixels and blended as premultiplied colours.
- The width brushed is the tile's height, bounded by `max_half_width`, 64 steps of the line's
  half width. The pattern's position along the line starts at 0 for each call and carries over
  every segment and part (`m_start`), each segment moving it by its length in subpixels.
- `msDrawShape` clips a line before it draws it, to the image grown by the class's `clip_buf`
  (`src/mapdraw.c`, 2463-2558): the largest size or width of the class's styles, scaled, plus 1.
  A tile's height never passes it. A layer with `PROCESSING "POLYLINE_NO_CLIP"` is not clipped,
  and AGG saturates its coordinates instead. Forager's `symbolize` reproduces the clip.

Two accepted ADRs leave this open. [ADR-0026](0026-agg-blender-and-resampler.md) names "a line
patterned with an image (`agg2RenderLineTiled`)" out of its scope as "a decision of its own".
[ADR-0044](0044-truetype-marker-glyph-through-a-port.md) names "a TrueType symbol along a line
with a `GAP` of 0" out of its scope. Neither decides anything about the path, so this ADR takes
it up without superseding either. [ADR-0025](0025-agg-rust-rasterizer.md) has agg-rust rasterize
the display list behind three safeguards. [ADR-0026](0026-agg-blender-and-resampler.md) has
`render-agg` blend and resample with ports of AGG 2.4 while agg-rust rasterizes.
[ADR-0027](0027-clip-to-the-image.md) has every renderer clip through `render`'s `clip.rs`.
[ADR-0029](0029-pixmaps-held-once-per-render.md) counts every tile against Q-9's 256 MiB.

agg-rust 1.2.0 ports the outline-image path. Read against the AGG 2.4 MapServer bundles
(2026-11-16), it differs in four places:

1. **The bilinear pattern filter** (`pattern_filters_rgba.rs`, 88-110) leaves out the rounding
   term `line_subpixel_scale * line_subpixel_scale / 2` that AGG adds before `>> 16`
   (`agg_pattern_filters_rgba.h`, 70). Nearly every pattern pixel comes out one lower in some
   channel. The `PatternFilter` trait is public, so a filter of one's own replaces it.
2. **`line_parameters::divide`** recomputes `vertical` and `inc` for each half, where AGG copies
   them (`agg_line_aa_basics.h`, 115-135). This differs only for a segment longer than 1024
   pixels close to 45°.
3. **`iround`** rounds half away from zero. MapServer's AGG rounds with `lrint`, half to even,
   as [ADR-0026](0026-agg-blender-and-resampler.md) records. The two differ only on an exact
   half: in `bisectrix` at a join, and in the distance interpolator's steps. Coordinates fed in
   as integers avoid it in `line_coord_sat`.
4. **`line_coord_sat`** is not applied by `move_to_d` and `line_to_d`; the integer `move_to`
   and `line_to` take saturated coordinates instead.

The rest is the same arithmetic: `line_image_pattern::create` and its dilation, the line
interpolator, the distance interpolator, `line3`, the miter-accurate join, no round caps, the
vertex filter, and the flush of the last part. agg-rust reaches the pixels through its
`PixelFormat` trait. `render-agg`'s port of AGG's pixel format already implements that trait
with `copy_or_blend_pix` for `blend_pixel` and `blend_color_hspan`, the two calls the renderer
makes.

agg-rust panics where C++ is undefined. A negative pattern index panics once the pattern's
position passes `i32`, after some 8.4 million pixels of path in one `add_path`. A pattern of
width 0 panics as well. MapServer's own C++ is undefined there.

No case of msautotest's `misc` and `renderers`, in scope or out, draws such a line: a scan of every
line style with a symbol and without a `GAP` found only styles with a `GEOMTRANSFORM`, which
place markers. So the harness of G-1 shows no gain, and MapServer's pixels for this path have to
come from elsewhere.

## Decision

We will brush a line style's line with its symbol's tile where the style's `GAP` is 0, as
`agg2RenderLineTiled` brushes it, through agg-rust's outline-image rasterizer.

- **Which styles.** A line style of a vector, ellipse, pixmap, or TrueType symbol with a `GAP` of
  0 must take this path. A hatch, an SVG symbol, symbol 0, and a simple symbol must keep their
  present drawing.
- **What goes.** `symbolize` must no longer draw such a style as a plain line in its `COLOR`. The
  warning that a pixmap is not tiled along a line must go.
- **The tile.** `symbolize` must ask for the tile as `msDrawLineSymbol` asks `getTile` for it:
  - its sides are `MS_NINT(sizex × scale)` and `MS_NINT(sizey × scale)`, or the extent truncated
    at a scale of 1, at least 1 each;
  - it asks for no seamless mode;
  - each kind is drawn once at the tile's centre, as a polygon's tile draws it;
  - a TrueType symbol's glyph is drawn there as
    [ADR-0044](0044-truetype-marker-glyph-through-a-port.md) draws it.
- **The cache.** The tile must go through the render's tile cache under the key a polygon's tile
  takes. A cached tile of that key must serve the line, a seamless one included, as
  `searchTileCache` lets it.
- **Its bounds** (Q-9). The tile must be held to the bound on a polygon's tile, 4096 pixels a
  side. The tile cache must count the tile and the copy `LineImagePattern` makes of it against the
  256 MiB of [ADR-0029](0029-pixmaps-held-once-per-render.md), before either is built.
- **What is not drawn.** Where a bound refuses the tile, the style must draw nothing on the line.
  The layer must warn as for a polygon's tile: `pixmap-not-drawn` for a pixmap, `symbol-not-drawn`
  for the others, once per symbol. A pixmap whose image does not load must warn
  `pixmap-not-drawn`. A TrueType glyph that cannot be drawn must draw nothing and warn as
  [ADR-0044](0044-truetype-marker-glyph-through-a-port.md) has it warn.
- **The drawing operation.** `render` must carry the line as a drawing operation of its own. It
  holds the paths of one shape in one style and a counted reference to the tile. Its start is 0,
  as at each call of `agg2RenderLineTiled`.
- **The rasterizer.** `render-agg` must draw the operation with agg-rust's `RasterizerOutlineAa`,
  `LineImagePattern`, and outline-image renderer. It must set no clip box, a scale of 1, and a
  start of 0.
- **The filter.** `render-agg` must sample the tile through a port of AGG 2.4's
  `pattern_filter_bilinear_rgba8`, its rounding term included. It must read the tile's
  premultiplied pixels as they are stored.
- **The coordinates.** `render-agg` must feed each vertex as an integer: `line_coord_sat`
  computed with `lrint` from `style-ir`'s `c` module.
- **The blend.** The rasterizer must blend through the pixel format of
  [ADR-0026](0026-agg-blender-and-resampler.md).
- **The clip.** `render-agg` must clip the line through `clip.rs`
  ([ADR-0027](0027-clip-to-the-image.md)). The image is grown by the reach the operation carries:
  the class's `clip_buf`, which no tile's height passes. A line `symbolize` clipped is then left
  whole.
- **The start across a cut.** Where `clip.rs` cuts a line, `render-agg` must move the start as AGG
  moves it over the part cut away. That is the sum of each segment's length in subpixels, `lp.len`
  between the vertices `line_coord_sat` gives.
- **No panic** (Q-3). `render-agg` must keep the start within `i32`. A wrapper of agg-rust's
  `OutlineAaRenderer` trait must reduce it by whole widths of the tile after each segment, which
  moves no pixel.

**Out of scope:**

- the residual differences of agg-rust from AGG 2.4 that the Context names: `divide`, and `iround`
  on a half;
- the pixels of a segment `clip.rs` cuts, whose new end is rounded to the clip's edge;
- a line style with a `GAP` other than 0, which places markers;
- a hatch or an SVG symbol along a line;
- a label's or a legend's line;
- Cairo's renderers, which have no `renderLineTiled`.

## Alternatives considered

- **Port AGG 2.4's outline-image path into `render-agg`.** A port of `line_image_pattern`,
  `renderer_outline_image`, `rasterizer_outline_aa`, and the line basics, some 1,900 lines of
  C++, with `lrint` for `iround`. It removes the residual differences, `divide` and the rounded
  halves. It also rasterizes a display item outside agg-rust, against the decision of
  [ADR-0025](0025-agg-rust-rasterizer.md), so it would need a successor of that ADR. No case in
  scope shows the residual differences. Not chosen for the size of the port and the supersession,
  against a difference no case is known to reach. Revisit if a case of the harness shows one.
- **agg-rust's path unchanged, its own bilinear filter included.** It is the least code. Its filter
  truncates where AGG rounds, which shifts nearly every pattern pixel by one in some channel.
  Not chosen: the filter is the largest difference, and replacing it costs one small type.
- **Fix agg-rust upstream and wait for a release.** The differences in `divide` and in `iround`
  are internal to the crate. A pull request upstream would remove them for everyone. Waiting
  blocks the path on a release outside the project's control. Kept as a follow-up rather than a
  precondition.
- **Fill the line's stroked outline with the tile, as a polygon's tile fills it.** The renderer
  can already do it, but the tile then stays aligned to the image, not to the line. It does not
  turn with each segment, so it is not MapServer's brush. Rejected.
- **Stamp the symbol along the line every tile's width, as markers.** It reuses the line markers.
  It places whole symbols, where MapServer's pattern is sampled and cut by the line's own width at
  each pixel. Joins and the pattern's continuity differ visibly. Rejected.
- **Keep the plain line and warn for every kind**, as `pixmap-not-drawn` warns today. This needs
  no ADR. It is the state that the maintainer's decision of 2026-11-16 to take the path up
  replaces.

## Sources / Prior art

- MapServer rel-8-6-6, `target/ms-src`: `src/maprendering.c` (`msDrawLineSymbol` 600-730,
  `getTile` 222-300, `computeSymbolStyle` 37-77, `searchTileCache` 83-97); `src/mapagg.cpp`
  (`agg2RenderLineTiled` 250-273, its pixel format 86-97); `src/mapagg.h` (`line_adaptor`).
- The AGG 2.4 MapServer bundles, `src/renderers/agg/include/`: `agg_renderer_outline_image.h`
  (`line_image_pattern` 106-177, `max_half_width` 478-480, the line interpolator 500-560),
  `agg_pattern_filters_rgba.h` (67), `agg_rasterizer_outline_aa.h`, `agg_line_aa_basics.h`
  (104-121).
- agg-rust 1.2.0, crates.io: `src/renderer_outline_image.rs`, `src/pattern_filters_rgba.rs`
  (88-110), `src/rasterizer_outline_aa.rs` (207), `src/line_aa_basics.rs` (`divide`),
  `src/basics.rs` (`iround`), `src/renderer_base.rs` (524-567).
- `crates/render-agg/src/pixfmt.rs`: the port of AGG's premultiplied pixel format under
  [ADR-0026](0026-agg-blender-and-resampler.md), and its `blend_pixel` and `blend_color_hspan`.
- msautotest at the pin, `misc` and `renderers`: a scan of the line styles with a symbol and
  without a `GAP`, 2026-11-16.

## Consequences

- Positive: a vector, ellipse, pixmap, or TrueType symbol along a line without a `GAP` is drawn as
  MapServer draws it, instead of a line in a colour MapServer does not draw. A pixmap there loads
  its image, so a missing file is reported as any other image's.
- Positive: the tile cache and its bounds serve lines as they serve polygons. No second cache and
  no second bound on memory.
- Negative: the residual differences of agg-rust stay. A segment past 1024 pixels near 45°, or a
  half rounded at a join, can differ from MapServer by a pixel's shade.
- Negative: a line of a layer with `POLYLINE_NO_CLIP` is cut by `clip.rs` where MapServer only
  saturates it. The segment cut is drawn from a rounded end, without the join before it. The
  start carried keeps the rest of the line's pixels.
- Negative: the render path grows by a drawing operation, a filter, the start's carry across the
  clip, and the wrapper that holds the start. agg-rust's outline-image code enters the bundle
  (Q-5). Its size is not measured here; the implementation measures it under the 3 MB budget.
- Follow-ups:
  - MapServer's pixels for this path, recorded through the noise workflow's build of MapServer,
    since no case of the harness has one.
  - Q-9 names the bound on "the tile a polygon is filled with". Its wording to cover a line's tile
    is a specification change, which the maintainer decides with this ADR.
  - The glossary's *Fill tile* covers polygons only; the term for the tile a line is brushed with
    is settled with the implementation.
  - Reports to agg-rust of the filter's rounding term, `divide`, and the missing saturation.

## Enforcement

Tests cite this ADR with `Verifies:`:

- a test in `crates/symbolize` that a line style of each kind with a `GAP` of 0 gives the
  line's drawing operation, not a plain line;
- a test in `crates/symbolize` of that tile's size, `MS_NINT(sizex × scale)` by
  `MS_NINT(sizey × scale)` pixels, and the truncated extent at a scale of 1;
- a test in `crates/symbolize` that a polygon's cached seamless tile serves a line of the same
  key;
- a test in `crates/symbolize` that a line's tile past 4096 pixels a side draws nothing and warns;
- a test in `crates/symbolize` that a line's tile past 256 MiB with its copy draws nothing and
  warns: `pixmap-not-drawn` for a pixmap, `symbol-not-drawn` for the others;
- a test in `crates/render-agg` of the filter, whose expected bytes are derived by hand from AGG
  2.4's `pixel_high_res` with its rounding term;
- a test in `crates/render-agg` that a line within the operation's reach is left whole by the
  clip;
- a test in `crates/render-agg` that a line cut by the clip draws the pixels of the whole line on
  the image, away from the segment cut;
- a test in `crates/render-agg` that a path whose length passes `i32` in the start draws without
  a panic;
- a test map of `crates/cli/tests/` drawn natively and on wasm32 alike (G-9), with a line of each
  kind.

The renderer's fuzz target draws the new drawing operation
([ADR-0025](0025-agg-rust-rasterizer.md)).
