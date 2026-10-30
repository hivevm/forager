# `OPACITY`, `COMPOSITE`, `OUTLINEWIDTH`, and the transparent image

**Where:** `STYLE`. **IR:** the style's `opacity`, percent, and `outline_width`, pixels; each a
number or a [binding](binding.md).

**Meaning.** `OPACITY` is how opaque the style draws, 0 to 100, default 100; `loadStyle` in
`src/mapfile.c` truncates a number to an integer and clamps it to 0…100, and the frontend stores
the result. An opacity below 100 replaces the alpha of the style's colours, a `#rrggbbaa`'s
included, with `opacity × 2.55` rounded (`loadStyle` at its `END`, and `bindStyle` in
`src/maputil.c` for a bound one); an opacity of 100 leaves the colours' alpha as written.

The engine applies it as it binds the style to a feature: a bound opacity is the property's
number rounded as `bindIntegerAttribute` rounds it, 100 where the property is empty, and is not
bounded; `opacity × 2.55` is rounded by `MS_NINT`, `lrint` in the build at the pin, so 50 gives
127.

**Where:** `LAYER`'s `COMPOSITE` block. **IR:** the layer's `opacity`, percent, a number.

**Meaning.** A layer has no `OPACITY` of its own at the pin: `loadLayer` in `src/mapfile.c`
refuses one, and a layer's opacity is its `COMPOSITE` block's, which `loadLayerCompositer` reads as
an integer from 0 to 100, default 100. `msDrawLayer` in `src/mapdraw.c` skips a layer whose one
block has an opacity of 0; otherwise it draws the layer into a transparent image of its own,
created with `msImageCreate`, and lays that image over the map once for each block
(`msCompositeRasterBuffer`), at `opacity × 2.55` rounded, through AGG's `blend_from`
(`aggCompositeRasterBufferNoPixman` in `src/mapagg.cpp`; the build with pixman composites through
it instead). The frontend carries one block that composites `src-over`, MapServer's default
`COMPOP`, without a `COMPFILTER`; another `COMPOP`, a `COMPFILTER`, or a layer of several blocks is
not yet supported and is ignored with a warning, and the layer is drawn straight onto the map.
The layer's labels go to the label cache and are drawn over every layer at full opacity. The image
of the layer keeps the `refpt` its creation leaves, its top left corner, which a hatch counts its
lines from (`msHatchPolygon`), where the map's image has it at the map's origin.

**The transparent image.** `msPostMapParseOutputFormatSetup` and `msSelectOutputFormat` in
`src/mapoutput.c` select the output format `IMAGETYPE` names, by the MIME type of the map's
`OUTPUTFORMAT` blocks first and by their names next, the first block's without an `IMAGETYPE`,
and a built-in format where none matches. `loadOutputFormat` makes a block of `IMAGEMODE RGBA`
transparent unless its `TRANSPARENT` says otherwise, and a JPEG one never is
(`msOutputFormatValidate`). The image of a transparent format starts with no colour, whatever the
map's `IMAGECOLOR` (`agg2CreateImage`), and the frontend gives the map the background `#00000000`
then; nothing else of the output format is carried. The renderer blends premultiplied colours as
MapServer's AGG pixel format does and divides the alpha out of each pixel as `saveAsPNG` in
`src/mapimageio.c` does for an RGBA image, truncating. It does so for every image: for an RGB
format, `saveAsPNG` writes the premultiplied colours without their alpha instead. The two differ
only where a pixel is not opaque, which on MapServer's RGB image never happens, as its background
is opaque (`msPrepareImage` sets the alpha of `IMAGECOLOR` to 255); in Forager, a translucent
`#rrggbbaa` `IMAGECOLOR`, which the frontend keeps, gives such pixels.

A layer's `opacity` that is not a finite number is taken as none; one outside 0 to 100, which a
mapfile cannot give but a style document can, is bounded to it with a warning.

`OUTLINEWIDTH`, at least 0, default 0, outlines a line of a `LINE` layer as RFC 49 states:
`msOutlineRenderingPrepareStyle` in `src/maprendering.c`, called from `msDrawVectorLayer` in
`src/mapdraw.c`, widens the line by twice the `OUTLINEWIDTH`, draws it in the `OUTLINECOLOR`
first, and the line in its `COLOR` over it. The
outline of a symbol is not it: `computeSymbolStyle` draws a symbol's outline as wide as the
style's `WIDTH` ([`width.md`](width.md)).
