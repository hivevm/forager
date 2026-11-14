# ADR-0036 words two cases of `not_drawn` otherwise than the code reads them

Observed: ADR-0036 gives the reason `literal` to an operation of literals that folds to no finite
number, while the frontend gives it to a single literal past a double too, such as `1e999`, which
`atof` reads as infinite. And ADR-0036 has a reader of an older minor read a `not_drawn` reason it
does not know as not drawn, with a warning naming its path, while the closed list of reasons has
none for it: the tree holds it as `syntax`, so the engine's later warning names `syntax`. Both are
written down in `docs/semantics/style-symbol.md`. Review of ticket revision-2026-11-13/01.

The maintainer chose on 2026-11-14 to keep the code and to state both cases in ADR-0036 at the next
normalization rather than supersede it now.

Where: docs/adr/0036-typed-geomtransform.md (Decision, Versions), crates/style-ir/src/transform.rs,
crates/style-ir/src/lenient.rs, crates/mapfile/src/expression/transform.rs.
