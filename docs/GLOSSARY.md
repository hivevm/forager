# Glossary — Forager

> The **vocabulary** of this project: the terms everyone, humans and agents alike, uses in code,
> comments, documentation, and ADRs. It defines words and decides nothing. Whoever resolves a term
> adds it here in the same change ([ADR-0001](adr/0001-agent-governance-model.md)).

One term per concept, only terms specific to this domain. Say what a term *is* in one or two
sentences, and list under _Avoid:_ the synonyms it displaces, where there are any. A term the
specification, an ADR, or the code uses without an entry here is a gap to close, not a convention
to invent.

- **Adapter** — The implementation of a port for one kind of source or resource, such as
  Shapefile, GeoJSON, or FlatGeobuf; it depends on the ports and the style IR, never on the core.
  _Avoid:_ driver, provider, plugin.
- **Base context** — What the host gives the core to resolve relative paths against: the root
  inside which every resource path must stay.
- **Binding** — A style property that takes a feature's property, as a mapfile's `[name]` binds
  an attribute: `{"binding": name}` in the style IR, read with MapServer's rules, not as an
  expression ([`semantics/binding.md`](semantics/binding.md)).
- **Core** — The crates that turn a style IR and features into pixels: no I/O, no C code, no
  MapServer syntax, and buildable for `wasm32-unknown-unknown`.
- **Diagnostic** — A message from loading or rendering, with a severity, a code, a text, and a
  source span where one is known; errors from user data are reported as diagnostics, never as
  panics.
  _Avoid:_ warning (for the general concept), log message.
- **Display list** — The ordered sequence of drawing operations the core hands a renderer, each
  tagged with the feature it came from.
- **Expression** — A condition or value over a feature's properties in the style IR: a CQL2
  tree ([ADR-0019](adr/0019-cql2-expressions.md)) the mapfile frontend translates from an
  `EXPRESSION`, a `FILTER`, a `TEXT`, or an attribute binding. Its semantics per parameter are in
  [`semantics/`](semantics/README.md).
- **Feature source** — The port through which the host supplies a layer's features, implemented by
  an adapter the host binds to the layer by its name; the engine asks it for the features in view.
- **Format version** — The `MAJOR.MINOR` version of the style document format, independent of
  crate versions and of the core's API.
- **Host** — The program that embeds the core and owns all I/O: the CLI, a browser page through
  the WASM bindings, or a server.
- **Label cache** — MapServer's deferred placement of labels: labels are collected over all layers
  and placed in a separate pass, after the geometry.
- **Mapfile** — A map definition in UMN MapServer's text syntax, read only by the mapfile
  frontend.
- **Mapfile frontend** — The crate that parses a mapfile, its expressions, and its symbol
  definitions, resolves MapServer's quirks, and emits a style IR.
  _Avoid:_ mapfile parser (it does more than parse).
- **Metatile** — A block of tiles rendered together on the server so that labels can be placed
  across tile boundaries.
- **msautotest** — The conformance suite of the MapServer repository: mapfiles, data, and
  reference images.
- **Port** — A trait at the edge of the core through which the host supplies data and resources;
  the only place async appears.
- **Property** — A named value of a feature, which a mapfile calls an item and binds as
  `[name]`. _Avoid:_ attribute, item, outside mapfile syntax.
- **Pushdown** — Handing the part of a filter that an adapter can evaluate natively to that
  adapter; the core evaluates the rest.
- **Render mode** — What a layer's `TYPE` states: how its features are drawn (point, line,
  polygon, raster, labels only), independent of their geometry type.
  _Avoid:_ layer type, geometry type (for this concept).
- **Renderer** — What turns a display list into an image; the reference renderer draws with
  agg-rust into an RGBA image.
- **Resource loader** — The port through which the host supplies the bytes of files a style refers
  to — `INCLUDE`s, symbol sets, font sets, fonts, pixmaps — resolved against the base context.
- **Scale denominator** — The map's scale as MapServer computes it when drawing, `n` for 1:`n`;
  scale ranges and `SYMBOLSCALEDENOM` are written in it
  ([`semantics/scale-range.md`](semantics/scale-range.md)).
- **Scale factor** — What a style's sizes, widths, gaps, patterns, and offsets are multiplied by
  when a map is drawn, from its resolution, its scale, and the layer's size units, as MapServer
  computes it ([`semantics/scale.md`](semantics/scale.md)).
- **Source reference** — A layer's description of where its data lives — a kind and properties
  taken unparsed from `CONNECTIONTYPE`, `CONNECTION`, `DATA`, and related keywords — which a
  resolver binds to an adapter.
  _Avoid:_ connection, data source.
- **Source span** — The location in a mapfile or style document a diagnostic or IR element comes
  from: file, line, and column.
- **Style document** — The style IR serialized as YAML or JSON, with a `version` field naming its
  format version.
- **Style IR** — The data model of a map, its layers, classes, styles, symbols, and labels, free
  of MapServer syntax, with expressions as CQL2.
  _Avoid:_ model, AST (for the whole map).
- **Viewport** — The part of a map that is drawn: an image size and an extent. A page asks the WASM
  bindings for one, which overrides the mapfile's `SIZE` and `EXTENT`.
