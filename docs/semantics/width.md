# `WIDTH` (style)

**Where:** `STYLE`. **IR:** the style's `width`, a number in pixels or a [binding](binding.md).

**Meaning.** The width of a line, or of the outline of a polygon or symbol. `loadStyle` in
`src/mapfile.c` takes a number of at least 0, or a binding; a negative number is a load error, and
`WIDTH` takes no expression.
