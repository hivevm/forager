# A label's style of a transform name MapServer does not know may draw at the label point

Observed while working ticket revision-2026-11-11/09 (one diagnostic for a label left out for its
style's transform). The frontend leaves out a label whose style has a `GEOMTRANSFORM` that names no
transform, such as `"foo"`, with `label-not-drawn`, and its class's other labels with it
(`crates/mapfile/src/translate/label.rs`, `label_style`). `docs/semantics/label.md` says
`msDrawLabelCache` fails the map's draw on such a name.

At the pin, `msStyleSetGeomTransform` (`src/mapgeomtransform.c`, 58-64) sets an unknown name's type
to `MS_GEOMTRANSFORM_NONE` and only records an error, and `loadStyle` (`src/mapfile.c`, 2831-2843)
does not check it. `loadLabel` (`src/mapfile.c`, 2058-2061) then gives a style of
`MS_GEOMTRANSFORM_NONE` its default, `MS_GEOMTRANSFORM_LABELPOINT`. No check of the error list
follows in `src/mapfile.c`. So MapServer seems to draw such a style at the label point, as if it
had no `GEOMTRANSFORM`. A later `GEOMTRANSFORM` in the same style replaces the type either way.

Not confirmed: whether a caller such as mapserv turns the recorded error into a failure. An
expression (`MS_GEOMTRANSFORM_EXPRESSION`) is not affected by this reading.
