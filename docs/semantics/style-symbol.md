# `SYMBOL` (style) and `GEOMTRANSFORM`

**Where:** `STYLE`. **IR:** the style's `symbol` and `geom_transform`.

**Meaning.** `SYMBOL` names the symbol the style draws: by its name, by MapServer's index into the
symbol set (at least 0; 0 is the default symbol), or bound to a property. The IR keeps the name
or the index as written until the symbol set itself is carried over and the reference is resolved
against it.

`GEOMTRANSFORM` draws the style on another geometry than the feature's: `start`, `end`,
`vertices` (every point of a line but its ends), and `centroid` are carried over, written quoted
or bare. MapServer's other transforms (`bbox`, `labelpoint`, `labelpoly`, …) and its expressions
are not yet, and are a warning; the style draws the feature's own geometry.
