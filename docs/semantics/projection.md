# `PROJECTION`

**Where:** `MAP` and `LAYER`. **IR:** the map's `projection`, a proj4 string; the layer's
`projection`, either `proj4` with a proj4 string or `from_source`.

**Meaning.** The coordinate reference system of the map's extent and image, and of a layer's
features. A layer without one is in the map's projection, and MapServer then reprojects nothing
(`msProjectionsDiffer` in `src/mapproject.c` treats a projection without parameters as no
difference). Reprojection itself is not yet drawn.

**Parameters.** MapServer hands every parameter to PROJ, so the frontend writes each one with a
leading `+` and joins them with spaces. `init=<authority>:<code>` is resolved by the resolver the
host passes in (ADR-0018); without one, or for a code it does not know, it is a load error. As in
PROJ's `pj_expand_init`, there is at most one `init=`, it is matched case-sensitively, and its
expansion follows the explicit parameters, which take precedence. An `init=` that names a file by
a path, such as `init=./data/epsg2:42304`, has its path as the authority and is resolved, or not,
like any other.

**`AUTO`.** A first parameter `AUTO`, in any case, leaves the projection to the layer's source:
`msProcessProjection` sets no PROJ object for it, and the source declares one when it opens — a
shapefile its `.prj` beside it (`msSHPLayerOpen` in `src/mapshape.c`), a raster what GDAL reads
(`src/mapraster.c`), an OGR layer its spatial reference. A source that declares none leaves the
layer in the map's projection. The IR holds this as `from_source`. On the map, where no source
declares one, `AUTO` is a warning and the map has no projection.
