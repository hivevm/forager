# Ticket: A style that names an image file draws it

## What to build

A style's or a label style's `SYMBOL` that names no symbol is read, as MapServer reads it, as an
image file. `classResolveSymbolNames` (`src/mapfile.c`, 3723–3762), run over every class once the
map and its symbol file are read (`resolveSymbolNames`, 7599–7609), resolves the name through
`msGetSymbolIndex` (408–426) with `try_addimage_if_notfound`; a name no symbol carries, matched
without regard to case from index 1, goes to `msAddImageSymbol` (`src/mapsymbol.c`, 386–462),
which appends a symbol named by the file name, SVG for a `.svg` extension and a pixmap otherwise
(408–416), its path against the map's directory (450–458). A later style naming the same file finds
the symbol the earlier one appended. No file is opened at load.

The frontend does the same into the style IR: it appends a pixmap or SVG symbol to the map's
symbols, in that order and after the symbol file's, its `IMAGE` resolved against the main
mapfile's directory inside the base context (Q-8), and the style refers to it by its index. The
style document's shape does not change. A name that starts with `http`, which MapServer fetches
with curl (418–449), is not fetched: the style keeps the name and draws nothing, and the frontend
warns, so the load has no error (G-1). A path refused by the base context is a diagnostic, as
`IMAGE` refuses one. `docs/semantics/style-symbol.md` and `symbol.md` say so, and drop the claim
that MapServer fails the load where the file is missing, which holds for a URL only.

## Acceptance criteria

- [ ] Given a mapfile with a symbol file and two styles naming `a.png` and `A.PNG` and one naming
  `b.svg`, when it is loaded, then the map's symbols end with a pixmap `a.png` and an SVG `b.svg`,
  in that order after the symbol file's, both styles of `a.png` refer to the first, and each
  `IMAGE` is the path beside the main mapfile.
- [ ] Given a style naming `../../outside.png` that leaves the base context, and one naming an
  absolute path, when loaded, then each is a diagnostic and no symbol is appended for it (Q-8).
- [ ] Given a style naming `http://localhost:8000/x.png`, when loaded and rendered, then the load
  has a warning and no error, and the style draws nothing.
- [ ] Given every mapfile of the msautotest suite, when loaded, then none has an error it did not
  have before (G-1).
- [ ] Given `point_pixmap`, when the harness renders it, then its outcome is the one the
  maintainer's answer to the spec's second open question decides, and the list says why.

## Read first

- [ADR-0007](../../adr/0007-harness.md), [ADR-0014](../../adr/0014-style-document-serde.md),
  [ADR-0020](../../adr/0020-style-document-compatibility.md),
  [ADR-0023](../../adr/0023-projection-resolver-and-init-files.md) (the base context of the
  frontend's paths).
- `docs/semantics/symbol.md`, `docs/semantics/style-symbol.md`; the frontend's translation of
  symbols and its tests of `IMAGE`; the conventions on ported behaviour. Seams: the frontend's
  tests, the G-1 load of the suite, the conformance harness.

## Blocked by

- docs/plan/pixmap-symbols/02-png-point-marker.md

## Mode

with the human: it resolves a resource path the mapfile names against the base context (Q-8),
which [ADR-0007](../../adr/0007-harness.md) never lets run unattended, and `point_pixmap`'s outcome
waits on the maintainer's answer about the URL.
