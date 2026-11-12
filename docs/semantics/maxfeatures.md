# `MAXFEATURES` and `maxfeaturestodraw`

A layer's `MAXFEATURES` is a number above 0, as `loadLayer` in `src/mapfile.c` reads it with
`getInteger`, truncated to an `int`; 0 or less fails the load. `msLayerGetMaxFeaturesToDraw` in
`src/maplayer.c` settles the most features a layer draws: its `MAXFEATURES`, replaced by the
`maxfeaturestodraw` key of the layer's `METADATA`, or else of the map's `WEB` `METADATA`, each read
by `atoi`, the key without regard to case. Where that is below 0, the selected output format's
`FORMATOPTION` `MAXFEATURESTODRAW` holds, read by `atoi`; below 0 still, there is no limit. The
style IR carries the result as the layer's `max_features`.

`msDrawVectorLayer` in `src/mapdraw.c` (1282–1286) stops at the first feature past the limit once
its class is found, in the order the source returns the features, so a feature no class takes does
not count. The features past it are not drawn, nor their labels.

A layer's `METADATA` is read for `maxfeaturestodraw` alone, and the output format, besides whether
the image starts transparent ([`opacity.md`](opacity.md)), for its `MAXFEATURESTODRAW` alone. Each
warns `unsupported-keyword`, naming what it is read for.
