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
projection as `msProjectRect` reprojects it: the points of its sides and of a diagonal, a hundred
along each, both projections taken with `+over`, a run of points that do not reproject cut at the
horizon, a grid where none of the ring is left, and every longitude where a polar projection's pole
lies inside it. A line or a ring is reprojected by `msProjectShapeLine`: a run of points that do not
reproject is cut at the horizon, found by halving each side to it down to 0.01 units; a line goes
on in a new part after the shape's others, a ring is closed along the horizon, and after a run of
no more than three such points its points move after the shape's other lines. Into longitude and
latitude from a projected layer, a vertex more than 180° from the one before is wrapped by 360°
where `msTestNeedWrap` finds the segment crossing the dateline. The points of a point shape are
reprojected by `msProjectLineEx`: all are left out where one does not reproject, except into
longitude and latitude from a projected layer, where such a point stays where it was. A warning
names the layer and how many points did not reproject (Q-3), and the points the horizon search adds
are bounded per layer (Q-9). `+over` keeps a longitude past ±180° on the way in and out.
Two parts of MapServer's reprojection are not ported, and each is stated. MapServer cuts the lines
of a line shape with GEOS where they cross the dateline from a source whose origin is the north
pole into longitude and latitude (`msProjectGetLineCuttingCase`); Forager draws them uncut and
warns `lines-not-cut`, naming the layer. The same cut for such a source into a map whose
projection is written `init=epsg:3857` is not even warned of, since the frontend expands `init=`
before the engine sees it. And `msProjectRect` remembers on the map's projection whether it is
polar (`is_polar`) from the first rectangle it projects, so only the first reprojected layer of a
polar map is asked for every longitude about the pole; Forager asks each such layer as MapServer
asks the first. A
projection the core does not implement leaves its layer undrawn with a warning. The core implements
longitude and latitude, `lcc`, `merc`, `tmerc` with `utm`, and `ortho`, a datum's shift by
`+towgs84`, and the datums WGS84 and NAD83, as PROJ 9.4.0 computes them (ADR-0037).

**A rotated map.** On a map with an [`ANGLE`](angle.md), `msMapSetFakedExtent` puts the map's
geotransform on the map's projection, and `msProjectionsDiffer` finds every layer where both sides
have parameters different from the map, so each is reprojected, a layer in the map's own projection
among them. `msProjectPointEx` moves a feature's point by PROJ's step, then by the inverse
geotransform into the frame of the image, y up. The area asked goes the other way:
`msProjectRect` samples the faked extent `0, 0, width, height`, moves each sample by the
geotransform, then by PROJ's step. Where both sides are the same single argument, such as
`init=epsg:21781`, MapServer creates no PROJ object, and where they are the same several, one from
the CRS into itself; either way only the geotransform moves a point, and the core runs no formula
for such a layer. A layer of `AUTO` whose source declares no projection, or one that cannot be
read, under a map of `AUTO`, is the first case. With a geotransform, `msProjectRect` skips its
polar branches, and the dateline wrap is measured in the frame's pixels, where the map is rotated
by other than a multiple of 180°. A layer without a projection, or under a map without one, is
not reprojected: its source is asked for the faked extent itself, and its coordinates are drawn
as the frame's pixels, unrotated, as MapServer draws them; Forager warns
`rotation-without-projection`, naming the layer, where MapServer is silent
(ADR-draft-map-rotation-in-the-reprojection).

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
one, leaves them as they are. The IR holds this as `from_source`; the engine asks the source bound
to the layer for the projection it declares (ADR-0039): a shapefile its
`.prj` (docs/semantics/shapefile.md), a GeoJSON source WGS84 longitude and latitude. A source that
declares none leaves the layer as MapServer leaves it, and one whose declaration does not read
does too, with a warning naming the layer and the file, where MapServer writes a debug message.
On the map, where no source declares one, `AUTO` stays the map's one argument without a PROJ
object, as `msProcessProjection` leaves it; the IR holds it as the map's `projection_auto` (format
version 1.1). A layer without a
projection beneath it is not reprojected, and a layer in a projection differs from it and is drawn
in the longitude and latitude of its own datum, its base geographic CRS
(`msProjectCreateReprojector`); its source is asked for the map's extent taken back from there.
