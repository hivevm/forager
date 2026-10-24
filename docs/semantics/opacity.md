# `OPACITY` and `OUTLINEWIDTH`

**Where:** `STYLE`. **IR:** the style's `opacity`, percent, and `outline_width`, pixels; each a
number or a [binding](binding.md).

**Meaning.** `OPACITY` is how opaque the style draws, 0 to 100, default 100; `loadStyle` in
`src/mapfile.c` truncates a number to an integer and clamps it to 0…100, and the frontend stores
the result. `OUTLINEWIDTH` is the width of a symbol's outline, at least 0, default 0.
