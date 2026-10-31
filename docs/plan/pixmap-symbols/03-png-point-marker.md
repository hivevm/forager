# Ticket: Draw a PNG pixmap symbol at a point (tracer bullet)

## What to build

The first path through every layer: a point layer's style whose symbol, from the map or its symbol
file, is a `TYPE PIXMAP` with a PNG `IMAGE`, drawn unturned at its own size, as MapServer draws it,
from the CLI and from the browser bindings, with the image ticket 02's store gives.

- **Frontend.** Nothing changes: the style IR already carries the symbol's `IMAGE` as a resource
  path resolved beside the file that holds it (`loadSymbol`, `src/mapsymbol.c`, 239–250).
- **Engine.** The point mark of a pixmap symbol is the image, not the square: the display list
  carries the image placed at the point, shared with every other placement of it, not copied. A
  pixmap the store refuses leaves its style undrawn, the layer warns with a diagnostic naming the
  symbol, its path, and why, and the rest of the map draws: a deliberate deviation, decided by the
  maintainer, from MapServer, whose `msDrawMap` (`src/mapdraw.c`, 508–519) returns no map when a
  layer fails to draw. `docs/semantics/symbol.md` says so, beside what the engine now draws of a
  pixmap at a point.
- **Renderer.** Unturned and at scale 1, `agg2RenderPixmapSymbol` (`src/mapagg.cpp`, 693–700)
  blends the image with full cover at `MS_NINT(x - width / 2.)`, `MS_NINT(y - height / 2.)`; the
  reference renderer does the same, `MS_NINT` as ADR-0037 ports it, blended as ADR-0034 blends a
  premultiplied source. A style's colour and opacity do not reach the image
  (`computeSymbolStyle`, `src/maprendering.c`, 47–48).
- **Hosts.** The CLI reads the image from its base context; the browser bindings take it among the
  files a page passes, by the path the mapfile names. The bundle size of the bindings is measured
  with the PNG decoder in it (Q-5).
- The glossary gains the term the code names the decoded image by, if it has none.

A turned or scaled image is ticket 04; this one draws a pixmap whose scale is not 1, or whose
angle is not 0, as the square it draws today.

## Acceptance criteria

- [ ] Given a point layer with a 32 × 32 RGBA PNG pixmap symbol at a point whose pixel is
  (50.5, 41.5), when it is rendered, then the image's top-left pixel lies at (34, 26), `MS_NINT`
  rounding 34.5 and 25.5 half to even, its pixels blended premultiplied over the map, and the
  engine's reference image holds it; the positions are literals in the test, not the engine's.
- [ ] Given a pixmap whose file is missing and one past 4096 × 4096, beside a layer that draws,
  when rendered, then each pixmap's style is not drawn, the warning names the symbol and the path,
  and the map is drawn with every other layer and style.
- [ ] Given a pixmap file of 32 MiB and one byte, when the CLI renders the map and when the browser
  bindings render it with the file passed by the page, then on each host the style is not drawn,
  with the file bound's warning, and the rest of the map draws.
- [ ] Given one image placed at a thousand points, when rendered, then the display list holds one
  image, which every placement refers to.
- [ ] Given the browser bindings and a page that passes the image beside the mapfile, when
  rendered, then the image is drawn as the CLI draws it, byte for byte.

## Read first

- [ADR-0007](../../adr/0007-harness.md), [ADR-0009](../../adr/0009-ports-and-adapters-crates.md),
  [ADR-0015](../../adr/0015-agg-rust-rasterizer.md), [ADR-0028](../../adr/0028-text-stack.md)
  (Fonts), [ADR-0034](../../adr/0034-render-agg-blends-as-agg-2-4.md),
  [ADR-0037](../../adr/0037-c-number-reading-in-style-ir.md),
  [ADR-0038](../../adr/0038-browser-bindings-bind-geojson-only.md) (left as it is),
  [ADR-0039](../../adr/0039-the-clip-to-the-image-is-one-module-of-render.md); ADR candidate B of
  the spec.
- The spec `docs/plan/pixmap-symbols/spec.md`; `docs/semantics/symbol.md`; the convention on what
  a render holds per feature. Golden path: the vector symbol at a point; the bindings' test of a
  page's fonts. Seams: the engine's render tests with reference images, the CLI's and the bindings'
  tests.

## Blocked by

- docs/plan/pixmap-symbols/02-png-decode-and-limits.md
- docs/plan/revision-2026-10-31/01-symbolize-crate.md

## Mode

with the human: it enforces the file bound of Q-9 in both hosts, a resource limit, and carries the
decoded image in the display list, the rest of ADR candidate B; both
[ADR-0007](../../adr/0007-harness.md) keeps with the maintainer.
