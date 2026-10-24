# `SYMBOL` (style) and `GEOMTRANSFORM`

**Where:** `STYLE`. **IR:** the style's `symbol` and `geom_transform`.

**Meaning.** `SYMBOL` names the symbol the style draws: by its name, by MapServer's index into the
[symbols](symbol.md) (at least 0; 0 is the default symbol), or bound to a property. A name
resolves at import to the index of the first symbol of that name, without regard to case, as
`msGetSymbolIndex` in `src/mapfile.c` resolves it. MapServer reads a name it does not find as an
image file and fails the load where there is none; the frontend, which reads no image at load,
keeps the name and warns. An index past the last symbol is a warning too: MapServer draws nothing
for it.

`GEOMTRANSFORM` draws the style on another geometry than the feature's: `start`, `end`,
`vertices` (every point of a line but its ends), `centroid`, and, for a label's style,
`labelpoint` and `labelpoly` ([`label.md`](label.md)) are carried over, written quoted or bare
and compared without case (`geom_transform` in `crates/mapfile/src/translate.rs`). MapServer's
other transforms (`bbox`, …) and its expressions are not yet, and are a warning; the style draws
the feature's own geometry.
