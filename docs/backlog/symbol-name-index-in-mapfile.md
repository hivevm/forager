# The mapfile frontend finds a symbol by its name in a lookup of its own

Observed: ticket 04 of the revision of 2026-11-11 moved the engine's copy of msGetSymbolIndex's
match onto `named_index` of `symbolize` (crates/symbolize/src/symbol.rs), and
crates/arch-test/tests/symbol_names.rs now refuses a comparison of a symbol's name with
`eq_ignore_ascii_case` elsewhere. A third lookup remains: the frontend's `named` and
`push_symbol` in crates/mapfile/src/translate/symbol.rs keep a map from each name in lower case to
the position of the first symbol that carries it. That is the same match written another way, so
the arch test does not see it.

ADR-0008 lets `mapfile` depend on `style-ir` and `ports` only, so it cannot call `symbolize`. One
lookup for all three would live in `style-ir`, beside the symbols it searches, which moves the
place the ticket chose; the maintainer decides whether that is worth it. The map serves the
frontend's many lookups at import, so a shared function would take a name and the symbols, and the
frontend would give up its map or keep it as a cache keyed by that function's notion of a match.

Where: crates/mapfile/src/translate/symbol.rs (`named`, `push_symbol`),
crates/symbolize/src/symbol.rs (`named_index`), crates/arch-test/tests/symbol_names.rs, ADR-0008.
