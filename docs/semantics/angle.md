# `ANGLE` (style)

**Where:** `STYLE`. **IR:** the style's `angle`, degrees counter-clockwise, or a
[binding](binding.md).

**Meaning.** The rotation of the style's symbol, or of a line's pattern. `loadStyle` in
`src/mapfile.c` takes a number from −360 to 360, a binding, or `AUTO`; a number outside is a load
error. `AUTO`, which follows the direction of a line, is not yet carried over and is a warning.
