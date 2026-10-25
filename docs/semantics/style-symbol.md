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
`vertices` (every point of a line but its ends), `centroid`, `labelpnt` (the IR's `labelpoint`),
and `labelpoly` are carried over, written quoted or bare. `msStyleSetGeomTransform` in
`src/mapgeomtransform.c` takes the first of its names, in the order `start`, `end`, `vertices`,
`bbox`, `labelpnt`, `labelpoly`, `labelcenter`, `centroid`, that the value begins with, without
regard to case, so `'startpoint'` is `start`; a value that begins with none is no transform, and
the frontend warns. A label's style draws at its `labelpnt` unless it names another
([`label.md`](label.md)). MapServer's other transforms (`bbox`, `labelcenter`) and its
expressions are not yet carried over, and are a warning; the style draws the feature's own
geometry.

**Repeated directives.** A later directive replaces an earlier one, but for a binding:
`loadStyle` never frees a property's binding when a later directive gives it a number, and
`bindStyle` in `src/maputil.c` draws the binding, so `WIDTH [w] WIDTH 3` is bound to `w`. Of
`SIZE`, a later directive frees only an expression, which `bindStyle` evaluates after the binding:
`SIZE [a] SIZE ([b] + 1)` is the expression, `SIZE [a] SIZE ([b] + 1) SIZE 5` the binding `a`.
`OFFSET` and `POLAROFFSET` keep the binding of each of their two numbers so, and a later `OFFSET`
replaces a line offset of −99 or −999 ([`offset.md`](offset.md)).
