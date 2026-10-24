# `SYMBOLSET` and `SYMBOL`

**Where:** `SYMBOLSET` in `MAP`, naming a symbol file; `SYMBOL` blocks in `MAP` and in the symbol
file. **IR:** the map's `symbols`.

**Meaning.** The symbols the styles draw. The frontend reads the symbol file through the resource
loader, resolved against the main mapfile's directory inside the base context (Q-8), as
`loadSymbolSet` in `src/mapsymbol.c` resolves it against the map's path; it reads it within the
limits of `INCLUDE` (Q-9), and it must start with `SYMBOLSET`.

**Order and index.** MapServer numbers its symbols: 0 is its default symbol, a filled circle of
size 1; then come the map's `SYMBOL` blocks in the order they stand, which `loadMap` appends as it
parses them, then the symbol file's, which it appends after the whole map. The IR lists them in
that order without the default, so the symbol at position `i` has index `i + 1`.

**A symbol**, as `loadSymbol` reads it: `NAME`; `TYPE` `vector` (the default), `ellipse`,
`pixmap`, `simple`, `truetype`, `hatch`, or `svg`; `FILLED`; `ANCHORPOINT`, two numbers from 0 to
1, default the centre; `CHARACTER` and `FONT` for a TrueType symbol; `IMAGE` for a pixmap or SVG,
a resource path resolved beside the file that holds the symbol — the mapfile or the symbol file —
and not read at load (Q-8); `TRANSPARENT`, a palette index from 0 to 255; `POINTS`, at most 100
pairs. A pixmap or SVG without `IMAGE`, and a vector or ellipse without `POINTS`, are load errors.
`ANTIALIAS` is read and ignored, as MapServer ignores it.

**Vector symbols** are measured and shifted as `loadSymbol` does at its `END`: the extent is the
largest x and y of the points, a pair `-99 -99` lifts the pen and starts a new line, and where a
point is negative, or the anchor point is not the centre, every point is shifted so that the
smallest x and y are 0, and the extent with them. A style's size is the symbol's height drawn
(`msSymbolGetDefaultSize`: its extent's height, or its width where the height is 0), so a symbol
2 wide and 4 high drawn at size 8 is 4 wide and 8 high. An ellipse's one point is its width and
height.
