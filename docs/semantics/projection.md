# `PROJECTION`

**Where:** `MAP` and `LAYER`. **IR:** the map's `projection`, a proj4 string; the layer's
`projection`, either `proj4` with a proj4 string or `from_source`.

**Meaning.** The coordinate reference system of the map's extent and image, and of a layer's
features. A layer without one is in the map's projection, and MapServer then reprojects nothing
(`msProjectionsDiffer` in `src/mapproject.c` treats a projection without parameters as no
difference).

**Reprojection.** A layer whose projection differs from the map's is drawn in the map's, as
`msDrawShape` in `src/mapdraw.c` reprojects each shape by `msProjectShapeEx` before its
`GEOMTRANSFORM`, classes, and labels. Two projections differ as `msProjectionsDiffer` finds it:
where neither lacks parameters and they differ in number or in any one, compared in order as text;
the IR compares the proj4 strings its frontend wrote, `init=` expanded, where MapServer compares the
arguments as written. The layer's source is asked for the map's extent reprojected into the layer's
projection: the rectangle that holds the points of its sides, a hundred along each, as
`msProjectRect` samples them before it refines the edges. A point that does not reproject is left
out with a warning naming the layer, and with it a line left with fewer than two points, a ring
with fewer than three, and a feature left with nothing; MapServer finds a horizon along a line
instead, and wraps lines at the dateline, which Forager does not do yet. A projection the core does
not implement leaves its layer undrawn with a warning. The core implements longitude and latitude,
`lcc`, and `merc`, a datum's shift by `+towgs84`, and the datums WGS84 and NAD83, as PROJ 9.4.0
computes them (ADR-draft-reprojection-ported-in-proj). A layer of `AUTO` is not reprojected yet.

**Parameters.** MapServer hands every parameter to PROJ, so the frontend writes each one with a
leading `+` and joins them with spaces. `init=<authority>:<code>` is resolved by the resolver the
host passes in (ADR-0019); without one, or for a code it does not know, it is a load error. As in
PROJ's `pj_expand_init`, there is at most one `init=`, it is matched case-sensitively, and its
expansion follows the explicit parameters, which take precedence.

**Init files.** MapServer hands `init=<file>:<code>` to PROJ, which opens a name beginning with
`/`, `./`, or `../` as a path from the working directory and searches any other in its search
paths, `PROJ_DATA` first, before the working directory (`pj_open_lib_internal` in PROJ's
`src/filemanager.cpp`); msautotest runs each mapfile from its own directory. The frontend reads
a file whose name holds a `/`, such as `./data/epsg2` or `data/epsg2`, through the resource loader
against the main mapfile's directory inside the base context, as `INCLUDE` and `FONTSET` are
(Q-8), once per load and within the size limit of the mapfile (Q-9), and takes the code's
definition from it, in the format of PROJ's former `epsg` init file as PROJ reads it
(`get_init_string` in its `src/init.cpp`): `<code>` and the parameters up to the next `<`, across
lines, `#` starting a comment, the first definition of a code holding, and the code after the last
`:` of the reference. A file outside the base context, one that cannot be
read, a code it does not hold, and a definition that names a grid file or refers on with `init=`
are each the load error `projection-unresolved`, naming the file or the code. A name without a
`/`, such as `epsg2`, is an authority like `epsg` and goes to the resolver (ADR-0019).

**`AUTO`.** A first parameter that is `AUTO` and nothing else, in any case, leaves the projection to
the layer's source: `msProcessProjection` sets no PROJ object for it, and the source declares one
when it opens — a shapefile its `.prj` beside it (`msSHPLayerOpen` in `src/mapshape.c`), a raster
what GDAL reads (`src/mapraster.c`), an OGR layer its spatial reference. Where the source declares
none, the layer keeps `AUTO` without a PROJ object, which `msProjectionsDiffer` finds different from
the map's, and `msProjectCreateReprojector` in `src/mapproject.c` takes its coordinates as longitude
and latitude in the datum of the map's projection; only a map without a projection, or a geographic
one, leaves them as they are. The IR holds this as `from_source`. On the map, where no source
declares one, `AUTO` is a warning and the map has no projection.
