# Scale ranges: `MINSCALEDENOM` and `MAXSCALEDENOM`

**Where:** `LAYER`, `CLASS`, `LABEL`, `STYLE`; in `LAYER` and `CLASS` also the old spellings
`MINSCALE` and `MAXSCALE`, which `loadLayer` and `loadClass` in `src/mapfile.c` read into the same
fields. **IR:** `min_scale_denom` and `max_scale_denom` on the layer, the class, the label, and
the style.

**Meaning.** The map scales, as denominators, at which the element is drawn. The map's scale
denominator is computed when it is drawn (`msCalculateScale` in `src/mapscale.c`), from its extent,
its width in pixels, its `UNITS`, and its `RESOLUTION`; where it is not positive, no range applies.
The IR holds each bound as the mapfile writes it, and the engine compares, because MapServer
compares differently by element:

- **Layer** (`msLayerIsVisible` in `src/mapdraw.c`): drawn when the scale is above
  `MINSCALEDENOM` and, where `MAXSCALEDENOM` is above 0, at most `MAXSCALEDENOM`. A
  `MAXSCALEDENOM` of 0 bounds nothing.
- **Class** (`msShapeGetNextClass` in `src/maputil.c`, through `msScaleInBounds`): a feature
  takes the class when the scale is at least `MINSCALEDENOM` and below `MAXSCALEDENOM`; a
  `MAXSCALEDENOM` of 0 matches no scale. Before that, `msLayerIsVisible` skips the whole layer
  when no class has a scale above its `MINSCALEDENOM` (where that is above 0) and at most its
  `MAXSCALEDENOM` (where that is above 0).
- **Label** (`msGetLabelStatus` in `src/maputil.c`, `msScaleInBounds`) and **style**
  (`msDrawVectorLayer` and the `*LayerDrawShape` functions in `src/mapdraw.c`): drawn when the
  scale is at least `MINSCALEDENOM` and below `MAXSCALEDENOM`; a `MAXSCALEDENOM` of 0 draws at no
  scale.

An unset bound is MapServer's -1, which never excludes. Each value is a number of at least 0, as
`getDouble` checks it with `MS_NUM_CHECK_GTE`; a negative one is a load error. The layer's
`LABELMINSCALEDENOM` and `LABELMAXSCALEDENOM` are not yet carried.
