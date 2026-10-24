# `CLASSITEM`

**Where:** `LAYER`. **IR:** none of its own; it is the property inside each class expression that
needs one ([`EXPRESSION`](expression.md)).

**Meaning.** The attribute a class `EXPRESSION` of the string, regular-expression, or list kind
is compared against. MapServer looks it up when it draws (`msEvalExpression` in
`src/maputil.c`), so it holds wherever it stands in the layer, after its classes too; of several,
the last is kept, as `loadLayer` in `src/mapfile.c` overwrites it. The frontend writes it into
each such expression as `to_string` of the property.
