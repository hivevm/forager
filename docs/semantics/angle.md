# `ANGLE` (style)

**Where:** `STYLE`. **IR:** the style's `angle`, degrees counter-clockwise, or a
[binding](binding.md), and `auto_angle`.

**Meaning.** The rotation of the style's symbol, or of a line's pattern. `loadStyle` in
`src/mapfile.c` takes a number from −360 to 360, a binding, or `AUTO`; a number outside is a load
error. `AUTO` turns the symbol with the direction of the line it is drawn on: the IR's
`auto_angle`.

# `ANGLE` (map)

**Where:** `MAP`. **IR:** the map's `angle`, in degrees, absent for 0 (format version 1.1).

**Meaning.** The map is drawn rotated: the image's x axis points that many degrees
counter-clockwise of the map's, so its content turns clockwise. `loadMapInternal` in
`src/mapfile.c` takes a number from −360 to 360 and no binding; a number outside is a load error.
`msMapSetRotation` in `src/mapobject.c` rotates only by an angle other than 0, so the frontend
writes no field for 0. A style document's reader drops an angle outside that range with the
warning `value-out-of-range`.

**Drawing.** `msPrepareImage` in `src/mapdraw.c` computes the map's geotransform from the extent
grown to the image (`msMapComputeGeotransform`) and fakes the map's extent as `0, 0, width,
height` at a cellsize of 1 (`msMapSetFakedExtent`). Every layer with a projection is then
reprojected through the geotransform into that frame of the image, and drawn there:
[`PROJECTION`](projection.md) says how. The scale comes from the real extent, and the factor of a
size in units other than pixels from `msGetGeoCellSize` ([scale](scale.md)). In the frame, the
map's units are the image's pixels: a layer's `GEOMTRANSFORM` and its `[map_cellsize]`, the clip,
and the labels work in pixels, as in MapServer
(ADR-draft-map-rotation-in-the-reprojection). A rotated map's centre sits half a pixel off an
unrotated one's, as MapServer centres the geotransform on the image's width and height over two.

**Symbols and labels on a rotated map.** `pointLayerDrawShape` in `src/mapdraw.c` subtracts the
map's angle from each angle of a point layer's styles and labels that is not 0, draws the shape,
and adds it back (`:2050-2060`, `:2112-2120`). An angle of exactly 0 stays 0, so a symbol of
`ANGLE 0`, or of none, stands upright on the image, and so does one at the map's own angle, while
the shapes turn with the map. A label's angle so turned is the one the label cache tests it at.
`lineLayerDrawShape` and `polygonLayerDrawShape` turn a label's angle alone, not a style's
(`:2226-2227`, `:2324-2326`). A line's `PATTERN` follows the rotated line: its dashes count from
where msDrawShape's clip begins the line in the frame of the image ([line](line.md)).

**Not as MapServer.** `lineLayerDrawShape` and `polygonLayerDrawShape` subtract the map's angle
from a label's and never add it back, so in MapServer the turn accumulates from shape to shape: a
line layer's first shape is labelled at the label's own angle, since the subtraction comes after
its text is set up, and each later one turned once more; a polygon layer's labels are turned once
for the first shape and once more for each after it. Forager turns each shape's label once, as
`pointLayerDrawShape` does. The labels of line and polygon layers are not drawn yet; they will be
turned so.
