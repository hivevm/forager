# Ticket: Port msGetSymbolIndex's match of a name once, in style-ir

## What to build

From `docs/backlog/symbol-name-index-in-mapfile.md`. A symbol's name is matched without case in
`symbolize::symbol::named_index` (`crates/symbolize/src/symbol.rs`), which the engine calls, and
again in the frontend's `named` and `push_symbol` (`crates/mapfile/src/translate/symbol.rs`), a map
from each name in lower case to the first symbol that carries it. ADR-0008 keeps `mapfile` from
`symbolize`. With ADR-0036 accepted, the match is a small MapServer port that lives in
`style-ir::c`, which every crate may reach. Port it there, call it from both, keep the frontend's
map as a cache keyed by the port's notion of a match, and let
`crates/arch-test/tests/symbol_names.rs` refuse the match by name through `c_numbers.rs`'s list of
ports.

## Acceptance criteria

- [ ] Given a symbol named `Pin` and a style's `SYMBOL "pin"`, when the mapfile loads and renders,
  then both the frontend and the engine find it, by tests at their seams.
- [ ] Given a definition of the match outside `crates/style-ir/src/c.rs`, when the arch tests run,
  then they fail naming it.

## Read first

- Touches: `crates/style-ir/src/c.rs`, `crates/mapfile/src/translate/symbol.rs`,
  `crates/symbolize/src/symbol.rs`, `crates/arch-test/tests/symbol_names.rs`,
  `crates/arch-test/tests/c_numbers.rs`, `docs/ARCHITECTURE.md`
- `docs/adr/0036-c-ports-in-style-ir.md`, `docs/adr/0008-ports-and-adapters-crates.md`

## Blocked by

- docs/plan/revision-2026-11-13/03-c-ports-written-inline.md

## Mode

unattended: the tests decide it; one revert. Stop when the port's name or place would need a change
of ADR-0036's text.
