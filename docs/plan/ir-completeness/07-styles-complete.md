# Ticket: Carry styles completely, with MapServer's style quirks resolved at import

## What to build

A `STYLE` loads with every property that renders: `SYMBOL` (by name or index, kept as a
reference until ticket 08 resolves it), `ANGLE` (incl. `AUTO`), `OFFSET`, `GAP`, `INITIALGAP`,
`PATTERN`, `LINECAP`, `LINEJOIN`, `LINEJOINMAXSIZE`, `OPACITY`, `MINSIZE`/`MAXSIZE`,
`MINWIDTH`/`MAXWIDTH`, `OUTLINEWIDTH`, `POLAROFFSET`, and `GEOMTRANSFORM` where it names a
transform the IR can state (`start`, `end`, `vertices`, `centroid`; others warn). The quirks are
resolved at import: `OFFSET x -99` (MapServer's marker for a line offset parallel to the line),
`PATTERN` dash lengths scaling with the width, and lengths in pixels of the output image
(`RESOLUTION` over `DEFRESOLUTION`, 72 dpi each by default) and `SIZEUNITS`, as the spec's
*Units and scale* decides. Each formula comes from MapServer's source and stands on the
property's `docs/semantics/` page with that source.

## Acceptance criteria

- [ ] Given a style with each property above, when it loads, then each is in the IR and none is
  reported as unsupported.
- [ ] Given `OFFSET 3 -99`, when it loads, then the IR holds a perpendicular line offset of 3, not
  a vertical offset of −99.
- [ ] Given `PATTERN 10 5 END` with `WIDTH 2`, when it loads, then the dash lengths in the IR are
  what MapServer draws for them, by the formula on the semantics page.
- [ ] Given `RESOLUTION 144` and `SIZE 10`, when it loads, then the size in the IR is in output
  pixels by the formula on the semantics page.
- [ ] Given `SIZEUNITS meters` on a layer, when it loads, then the styles carry the scale factor
  the engine applies, and the semantics page states it.
- [ ] Given each of these, when the document round-trips through YAML and JSON, then it is equal.

## Read first

- The spec next to this ticket (*Units and scale*, *Bound values*);
  [ADR-0014](../../adr/0014-style-document-serde.md).
- MapServer `rel-8-6-6`: `loadStyle` in `src/mapfile.c`, `msDrawLineSymbol` and the
  scale-factor code in `src/mapdraw.c` and `src/maprendering.c`.
- Ticket 04's bound values; the frontend tests at the `load` seam.

## Blocked by

- docs/plan/ir-completeness/04-attribute-bindings-and-colour-range.md

## Mode

with the human: it changes the style document format ([ADR-0007](../../adr/0007-harness.md)), and
each quirk's resolution is a fidelity judgement (Q-4) the maintainer checks.
