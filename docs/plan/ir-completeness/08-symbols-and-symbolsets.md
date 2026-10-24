# Ticket: Carry symbols from SYMBOLSET files and inline SYMBOL blocks, and resolve style references

## What to build

The map's symbols load into the IR once, keyed by name: from the `SYMBOLSET` file, read through
the resource loader inside the base context (Q-8), and from inline `SYMBOL` blocks, with their
type (`vector`, `ellipse`, `pixmap`, `truetype`, `hatch`, `svg`), points, `FILLED`, `ANCHORPOINT`,
`CHARACTER` and `FONT`, and `IMAGE`. A style's `SYMBOL` by name or index resolves to that key at
import; an unknown symbol is a warning. A pixmap or SVG image is a resource path under Q-8, not
read at load. The quirk that a vector symbol's size is its height is resolved: the IR states a
symbol's extent so that a style's size means one thing. Writes the `docs/semantics/` pages of
`SYMBOLSET`, `SYMBOL`, and the style's `SYMBOL`.

## Acceptance criteria

- [ ] Given `SYMBOLSET "symbols.sym"` with three symbols, when the mapfile loads with a loader that
  has the file, then the IR holds the three, and a style naming one references it.
- [ ] Given `SYMBOL 2` in a style, when it loads, then it references the symbol MapServer numbers
  2 (index 0 is MapServer's default symbol), as the semantics page states.
- [ ] Given `SYMBOLSET "../x.sym"` or an absolute `IMAGE` path, when it loads, then it is refused
  with the Q-8 diagnostic (citing Q-8).
- [ ] Given a vector symbol 2 wide and 4 high and a style `SIZE 8`, when it loads, then the IR's
  symbol and size draw it 8 high and 4 wide, by the formula on the semantics page.
- [ ] Given a style naming an unknown symbol, when it loads, then a warning names it.
- [ ] Given the symbols, when the document round-trips through YAML and JSON, then it is equal.

## Read first

- Q-8 of `docs/SPECIFICATION.md`; the *Resource loader* and *Base context* glossary entries;
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0014](../../adr/0014-style-document-serde.md).
- MapServer `rel-8-6-6`: `src/mapsymbol.c` (`loadSymbolSet`, `msGetSymbolIndex`), and the vector
  symbol scaling in `src/maprendering.c`.
- The frontend's `INCLUDE` loading, the golden path for reading a file through the loader.

## Blocked by

- docs/plan/ir-completeness/07-styles-complete.md

## Mode

with the human: it changes the style document format and resolves resource paths, both kept with
the maintainer ([ADR-0007](../../adr/0007-harness.md)).
