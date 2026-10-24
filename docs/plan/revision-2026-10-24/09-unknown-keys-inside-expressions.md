# Ticket: Report an unknown key inside an expression or a bound as every other unknown field

## What to build

An unknown key inside an expression or a bound value is treated as ADR-0020 treats an unknown
field anywhere else: in a document of a newer minor it is dropped with an `unknown-field` warning
naming its JSON Pointer; in the current or an older minor it is a load error with the same path.
Today neither happens. `Expr` (`crates/style-ir/src/expression.rs`) and `Bound`
(`crates/style-ir/src/model.rs`) are `#[serde(untagged)]`; serde buffers their content and
`serde_ignored` never sees the keys an alternative skips. Measured on `main` ff5ccf0: the
document

```json
{"version":"1.0","map":{"layers":[{"filter":{"op":"=","args":[{"property":"a","glow":1},1],"flags":"x"},
 "classes":[{"styles":[{"width":{"binding":"w","glow":2}}]}]}]}}
```

loads without an error and without a warning, as `1.0` and as `1.9`. This is the open finding of
the review of #42 (G-4: "ignore its unknown fields with a diagnostic naming the field and its
location").

The fix stays inside the readers: for example, the alternatives with keys (`Call`, `Property`,
`Date`, `Timestamp`, `Binding`) read through types that refuse an unknown key, or the reader
compares the keys of each expression's value tree with those of the alternative it read. The
shape of the schema does not change; if it does, the schema file is regenerated in the same
change, under `1.0`.

## Acceptance criteria

- [ ] Given the document above as `1.0`, in JSON and in YAML, when it is read, then it is the
  load error `unknown-field` naming the path of one of its three unknown keys.
- [ ] Given it as `1.9`, when it is read, then it loads with one `unknown-field` warning per key,
  `/map/layers/0/filter/args/0/glow`, `/map/layers/0/filter/flags`, and
  `/map/layers/0/classes/0/styles/0/width/glow`, and the expression and the binding read as
  without them (citing ADR-0020).
- [ ] Given the existing tests of `style-ir` and the `style_document` fuzz target, when they run,
  then they pass unchanged.

## Read first

- [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0019](../../adr/0019-cql2-expressions.md),
  [ADR-0014](../../adr/0014-style-document-serde.md); G-4 of `docs/SPECIFICATION.md`.
- `crates/style-ir/src/document.rs` (`checked`, `lenient`) and its tests, the seam.

## Blocked by

- None (can start immediately)

## Mode

with the human: the style document's versioning is kept with the maintainer
([ADR-0007](../../adr/0007-harness.md)).
