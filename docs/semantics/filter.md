# `FILTER` and `FILTERITEM`

**Where:** `LAYER`. **IR:** the layer's `filter`, a CQL2 tree
([ADR-0030](../adr/0030-cql2-expressions.md)).

**Meaning.** Which of the source's features the layer reads at all, before any class is tried. A
layer without one reads every feature. `FILTER` takes the same four kinds of expression as
[`EXPRESSION`](expression.md); the string, regular-expression, and list kinds compare against
`FILTERITEM`, as a class's compare against `CLASSITEM`, and without one they never match. Like
`CLASSITEM`, `FILTERITEM` holds wherever it stands, and the last one is kept.

**Not carried over.** A `FILTER` written as a database's own query, which MapServer hands to the
source unparsed, and `%name%` runtime substitutions, are warnings, and the filter keeps `false`.
