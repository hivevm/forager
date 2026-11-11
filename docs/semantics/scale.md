# Scaling: resolution, units, `SYMBOLSCALEDENOM`, and the size and width bounds

**Where:** `RESOLUTION`, `DEFRESOLUTION`, and `UNITS` in `MAP`, `SIZEUNITS` and
`SYMBOLSCALEDENOM` (or its old spelling `SYMBOLSCALE`) in `LAYER`, `MINSIZE`, `MAXSIZE`,
`MINWIDTH`, `MAXWIDTH` in `STYLE`. **IR:** the map's `resolution`, `def_resolution`, and `units`,
the layer's `size_units` and `symbol_scale_denom`, the style's `min_size`, `max_size`,
`min_width`, `max_width`.

**Meaning.** The style IR holds sizes, widths, gaps, patterns, and offsets as the mapfile writes
them, and these parameters state how they are scaled when a map is drawn. MapServer computes the
scale while it draws, from the map's scale, so the frontend resolves nothing here; the engine
applies the formulas below.

**The scale factor** of a layer, `msPrepareImage` in `src/mapdraw.c`, which its classes, their
styles and labels, and the labels' styles inherit (`msUpdateClassScaleFactor`): the mapfile gives
none of them a `SIZEUNITS` of its own, as `loadClass`, `loadStyle`, and `loadLabel` read none, so
each is `MS_INHERIT`. The layer's factor is:

- `SIZEUNITS` other than `pixels`: the inches per size unit over the inches per map unit, divided
  by the size of a pixel in map units. The resolution does not enter it.
- else, with a layer's `SYMBOLSCALEDENOM` and a map scale: `SYMBOLSCALEDENOM / scale ×
  RESOLUTION / DEFRESOLUTION`.
- else `RESOLUTION / DEFRESOLUTION`. Both default to 72 dpi and take 10 to 1000, as `msLoadMap`
  checks. A style document's reader drops a `resolution` or `def_resolution` outside that range
  with the warning `value-out-of-range`, and the default applies.

The map's scale is its scale denominator as `msCalculateScale` in `src/mapscale.c` computes it.
So with `SYMBOLSCALEDENOM 25000`, a style of `SIZE 10` is drawn 10 pixels high at 1:25000, 20 at
1:12500, and 5 at 1:50000, at the default resolutions, before the bounds below.

The inches per unit are those of `inchesPerUnit` in `src/mapscale.c`, which
`msInchesPerUnit` reads: inches 1, feet 12, miles 63360, meters 39.3701, kilometers 39370.1,
nautical miles 72913.3858, decimal degrees 4374754. The map's `UNITS` defaults to meters
(`initMap` in `src/mapfile.c`).

**Applying it.** A symbol's size is `size × factor`, then bounded by `MINSIZE` and `MAXSIZE`,
each times `RESOLUTION / DEFRESOLUTION` (`computeSymbolStyle` in `src/maprendering.c`); the
defaults are 0 and 500. A line's width is `width × factor`, bounded by `MINWIDTH` and
`MAXWIDTH` the same way, defaults 0 and 32; a line's pattern, offset, and initial gap are each
multiplied by the bounded width over the width as written, or by 1 for a width of 0
(`msDrawLineSymbol`), and a symbol's gap by the bounded size over the size as written. Where the
lower bound exceeds the upper, a size takes the upper (`MAXSIZE` applies last) and a width the
lower (`MINWIDTH` applies last). A symbol's outline is the style's width, scaled and bounded as a
line's (`computeSymbolStyle`). A bound value or an expression is scaled after
it is evaluated. A line's outline pass, `msOutlineRenderingPrepareStyle`, widens the width by
twice the `OUTLINEWIDTH` over the scale factor and times the resolution factor, so that it grows
by the outline width at the resolution once scaled, and its bounds by twice the outline width as
written. A label's `SIZE` is `size × factor`, bounded by its `MINSIZE` and `MAXSIZE` times the
resolution factor (`msPopulateTextSymbolForLabelAndString` in `src/maplabel.c`); its `OFFSET`,
`BUFFER`, the outline's growth of its offset from a marker, and its styles are scaled by the
factor, its `MINDISTANCE` by the resolution factor (`msDrawLabelCache`). The size of a marker
under a label is `msGetMarkerSize`'s, the factor and the bounds as written.

`MINSIZE` and `MAXSIZE` take numbers of at least 0 and above 0, `MINWIDTH` and `MAXWIDTH` the
same, as `loadStyle` in `src/mapfile.c` checks; `SYMBOLSCALEDENOM` takes a number of at least 1,
as `loadLayer` checks. `SIZEUNITS` takes `pixels`, `inches`, `feet`, `miles`, `meters`,
`kilometers`, `nauticalmiles`, and `dd`; the map's `UNITS` the same without `pixels`.

The size of a pixel in map units is the map's cellsize, as `msGetGeoCellSize` takes it for a map
that is not rotated. Decimal degrees take no correction by latitude, as the build at the pin leaves
`ENABLE_VARIABLE_INCHES_PER_DEGREE` off.
