# Scaling: `RESOLUTION`, `DEFRESOLUTION`, `SIZEUNITS`, and the size and width bounds

**Where:** `RESOLUTION` and `DEFRESOLUTION` in `MAP`, `SIZEUNITS` in `LAYER`, `MINSIZE`,
`MAXSIZE`, `MINWIDTH`, `MAXWIDTH` in `STYLE`. **IR:** the map's `resolution` and
`def_resolution`, the layer's `size_units`, the style's `min_size`, `max_size`, `min_width`,
`max_width`.

**Meaning.** The style IR holds sizes, widths, gaps, patterns, and offsets as the mapfile writes
them, and these parameters state how they are scaled when a map is drawn. MapServer computes the
scale while it draws, from the map's scale, so the frontend resolves nothing here; the engine
applies the formulas below.

**The scale factor** of a style, `msUpdateClassScaleFactor` in `src/mapdraw.c`:

- `SIZEUNITS` other than `pixels`: the inches per size unit over the inches per map unit, divided
  by the size of a pixel in map units. The resolution does not enter it.
- else, with a layer's `SYMBOLSCALEDENOM` and a map scale: `SYMBOLSCALEDENOM / scale ×
  RESOLUTION / DEFRESOLUTION`.
- else `RESOLUTION / DEFRESOLUTION`. Both default to 72 dpi and take 10 to 1000.

**Applying it.** A symbol's size is `size × factor`, then bounded by `MINSIZE` and `MAXSIZE`,
each times `RESOLUTION / DEFRESOLUTION` (`computeSymbolStyle` in `src/maprendering.c`); the
defaults are 0 and 500. A line's width is `width × factor`, bounded by `MINWIDTH` and
`MAXWIDTH` the same way, defaults 0 and 32; a line's pattern, offset, and initial gap are each
multiplied by the bounded width over the width as written (`msDrawLineSymbol`), and a symbol's
gap by the bounded size over the size as written. A bound value or an expression is scaled after
it is evaluated.

`MINSIZE` and `MAXSIZE` take numbers of at least 0 and above 0, `MINWIDTH` and `MAXWIDTH` the
same, as `loadStyle` in `src/mapfile.c` checks; `SIZEUNITS` takes `pixels`, `inches`, `feet`,
`miles`, `meters`, `kilometers`, `nauticalmiles`, and `dd`.
