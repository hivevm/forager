# Ticket: The small deviations the port review left

## What to build

Found by areas 3, 4, and 5 of the port review (revision ticket 23, #66, #67, #70), each at
`rel-8-6-6`, each small, each changing what a mapfile yields:

- **A symbol file without its last `END`** (`SYMBOLSET SYMBOL TYPE HATCH END`) reads in MapServer:
  `loadSymbolSet` returns success at EOF (`src/mapsymbol.c:647-649`). The frontend reports
  `unterminated-block` and the load fails; the fix is in `Parser::block`.
- **Font names that differ only in case** are one key in MapServer (`src/maphash.c:113-157`, the
  last wins); the frontend keeps two. The IR's `fonts` then states that names match without
  case.
- **Script prefixes in a FONT list** (`ar:font`, `src/textlayout.c:339`) give a spurious
  `font-unknown` warning.
- **`OPACITY [o] OPACITY 50`:** MapServer applies the alpha for 50 at load and keeps it when the
  bound value is 100 (`src/mapfile.c:2794-2806`); not modelled.
- **ALIGN and POSITION bound:** where the bound value does not read, `bindLabel` keeps the
  earlier value (`src/maputil.c:~362-415`); the engine keeps the binding and warns
  `label-binding-unsettled`, which stays: reproducing MapServer's leftover value was dropped in
  the triage of 2026-11-07, since no case of the suite binds a value that does not read.
- **Not checked:** whether GDAL's `CPLReadLineL`, which `msLoadFontSet` reads with, ends a line
  at a lone CR.

## Acceptance criteria

- [ ] Given each case, when it loads, then it reads as MapServer reads it, or the ticket records
  with the maintainer why not, a test each.

## Read first

- The convention on behaviour ported from MapServer in `docs/CONVENTIONS.md`;
  `crates/mapfile/src/syntax.rs`, `crates/mapfile/src/translate/fonts.rs`, `style.rs`, `label.rs`.

## Blocked by

- None (can start immediately)

## Mode

with the human: the font names' case changes what the IR's `fonts` means
([ADR-0006](../../adr/0006-harness.md)).

## Maintainer's decision (2026-11-10)

As described: follow MapServer at the pin exactly; font names match without regard to case, the last
of two winning; a symbol file without its last `END` reads.
