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

**Not yet drawn.** The turn of a style's or a label's angle by the map's angle, which
`pointLayerDrawShape`, `lineLayerDrawShape`, and `polygonLayerDrawShape` apply: symbols and labels
on a rotated map are drawn at their own angle.
