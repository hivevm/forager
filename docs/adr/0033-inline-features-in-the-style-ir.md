# ADR-0033: A layer's inline features are a field of the layer in the style IR, read by the engine as the layer's source

- **Status:** 🟢 accepted
- **Date:** 2026-10-30
- **Deciders:** Markus Brigl
- **Applies to:** the inline features of a layer in `crates/style-ir`, the reading of `FEATURE` in `crates/mapfile`, and the source the engine makes of them in `crates/engine`

## Context

46 mapfiles of the msautotest suite draw features written into the mapfile instead of read from
a file: a layer's `FEATURE` blocks, 43 of them with `POINTS`, 3 with `WKT`, 20 with `TEXT`, and 4
with `ITEMS` named by the layer's `PROCESSING "ITEMS=…"`. Some 17 cases of the geometry category
(G-6) wait on them. The frontend knows the keyword and carries nothing of it, and the engine
reads features only from the sources a host binds to a layer by its name.

MapServer at the pin reads them so. `loadLayer` (`src/mapfile.c`) refuses a `FEATURE` before the
layer's `TYPE` and gives the shape the type of the layer: point, line, or polygon. `loadFeature`
adds each `POINTS … END` block as one part of the shape, an x read with `atof` and a y with
`getDouble`; splits `ITEMS` at `;`, keeping empty values; keeps `TEXT` as the shape's text; and
replaces the whole shape by what `msShapeFromWKT` makes of a `WKT`, through GEOS where MapServer is
built with it: a point, line, or polygon shape of the WKT's own type, the parts of a collection
gathered into one line shape. A layer with features is an inline layer whatever its `DATA` says
(`msInitializeVirtualTable`, `src/maplayer.c`), and its features are delivered in the order they
were written, each whose bounds meet the search rectangle. An inline layer's item names are those
of its `PROCESSING "ITEMS=…"` split at `,`, compared without regard to case, the first winning
(`msLayerGetItems`, `string2list`); a feature with fewer values than names gets `""` for the rest
(`msINLINELayerNextShape`).

The forces: the style document is the interface (G-3), so what a mapfile draws must be in it, or a
document converted from a mapfile draws less than the mapfile; a change to its shape follows
[ADR-0020](0020-style-document-compatibility.md) and stays with the maintainer
([ADR-0007](0007-harness.md)); the engine reads every feature through the port with the values of
[ADR-0026](0026-feature-properties-at-the-port.md); the browser build adds no dependency it does
not need (Q-5); and an input bounds its cost (Q-9), here through the mapfile's 16 MiB of text.

## Decision

We will carry a layer's inline features in the style IR as a field of the layer, each feature a
shape as MapServer's `loadFeature` leaves it, and have the engine read that field as the layer's
source, in place of any source a host binds to the layer.

- **Shape.** A feature holds its type (point, line, or polygon), its parts, each a list of
  coordinates, its properties, and its text where it has one; a polygon's parts are its rings, as
  MapServer's shape holds them, without an outer ring told from an inner one.
- **Properties.** The frontend names a feature's values by the layer's `PROCESSING "ITEMS=…"`,
  each name with the value at its place, `""` where the feature has fewer, a value past the names
  dropped; the style document holds them as a list of name and value, in that order. The engine
  delivers them as strings, found by name without regard to ASCII case, the first winning.
- **WKT.** The frontend reads a `WKT` with a reader of its own into the same shape, as
  `msGEOSGeometry2Shape` converts what GEOS parses: points, lines, polygons, their multi forms,
  and a collection's parts gathered into a line shape. A WKT it cannot read, an empty one, or one
  with Z or M coordinates is a load error at its span.
- **Format.** The field is optional and new, a minor change under ADR-0020; before the first
  release the current version's schema is regenerated and the version stays.
- **Source.** The engine makes an in-memory feature source of a layer's inline features, ids
  their order from 0, and uses it for that layer; a source a host binds to the layer is not read,
  and the engine reports that with a warning.

**Out of scope:** the inline feature's `TEXT` as a label's text, which the labels take up; a
layer's `GEOMTRANSFORM` and `TRANSFORM` on inline features; inline features of other connection
types, such as `GRATICULE`, which MapServer exempts; `PROCESSING` keys other than `ITEMS`.

## Alternatives considered

- **The frontend turns inline features into a GeoJSON text the existing adapter reads.** No new
  source in the engine, but the WKT would be converted twice, the spans of each feature lost, the
  feature's type would follow GeoJSON rather than the layer's `TYPE`, and the style document would
  carry a string of another format where it carries structure everywhere else.
- **The host binds inline features as it binds a file.** The engine would stay as it is, but a
  style document converted from a mapfile would no longer draw what the mapfile draws without the
  host repeating what the document should say (G-3).
- **The IR keeps the ports' `Geometry`.** It would spare a conversion, but the IR would depend on
  the ports, against [ADR-0009](0009-ports-and-adapters-crates.md), and a polygon would need its
  rings sorted into polygons that MapServer never sorts.
- **The `wkt` crate of georust reads WKT.** It reads more than MapServer's GEOS path accepts and
  pulls `geo-types` into the frontend and the browser build (Q-5); the WKT of the suite is three
  lines of the six types.
- **Item names kept on the layer and values by place on each feature.** Closer to MapServer's
  storage, but every reader of a style document would have to join them, and a document written by
  hand would name nothing where it is read.

## Sources / Prior art

- MapServer `rel-8-6-6`: `loadFeature` and `loadFeaturePoints`, and the `FEATURE` case of
  `loadLayer`, in `src/mapfile.c`; `msInitializeVirtualTable`, `msLayerGetItems`, `string2list`,
  `msINLINELayerNextShape`, and `msINLINELayerGetShape` in `src/maplayer.c`; `msShapeFromWKT` in
  `src/mapprimitive.cpp`; `msGEOSGeometry2Shape` in `src/mapgeos.c`.
- MapServer's mapfile reference, the `FEATURE` object of `LAYER`.
- The 46 mapfiles of the msautotest suite at the pin that hold `FEATURE`.

## Consequences

- Positive: a mapfile with inline features converts into a style document that draws them, and
  the cases that wait on them can be measured.
- Positive: the engine keeps one way of reading features, the port, whatever holds them.
- Negative / trade-offs: the frontend carries a WKT reader to keep in step with what GEOS accepts
  for the suite; WKT beyond it is refused, not approximated.
- Negative / trade-offs: a host that binds a source to an inline layer sees it ignored, as
  MapServer ignores the `DATA` of such a layer.
- Follow-ups: the feature's text as the text of its labels; `GEOMTRANSFORM` on inline features.

## Enforcement

A test of `crates/mapfile` loads a layer with `POINTS`, `WKT`, `ITEMS` named by `PROCESSING`, and
`TEXT`, and expects the shape, the properties, and the text this ADR states, and the load error of
an unreadable WKT; a test of `crates/engine` renders a layer with inline features and a host
source bound to it, and expects the inline features drawn, the source unread, and the warning.
Each cites this ADR with `Verifies: ADR-0033`.
