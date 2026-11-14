# Architecture — Forager

> The system **as it stands today**: the parts it is built from, what each is responsible for, and
> how they fit together. It holds no rule and no decision. It names the ADR behind each structural
> choice; where it disagrees with an accepted ADR, the ADR is right and this document is stale. It
> is updated in the same change as the structure it describes
> ([ADR-0001](adr/0001-agent-governance-model.md)).
>
> **Last design revision:** 2026-11-13, due after 20 changes. The revision that took the count
> moves the date; the number is this project's to set; a sensor counts the changes outside
> `docs/` since the commit that set the date and says when the next is due
> ([ADR-0005](adr/0005-feature-layer.md)).

## Context

What sits outside the system and what crosses its boundary. A diagram earns its place here more
than anywhere else, and none beats one that has stopped being true.

A host embeds the core and owns all I/O: the CLI reads files, and the browser bindings take what a
page passes in. The host hands the mapfile frontend a resource loader rooted at the base context
and a projection resolver; the frontend returns a style document and diagnostics. To render, the
host binds a feature source to each layer by the layer's name, which a layer that holds its
features inline does not need, and hands the engine a renderer;
the engine returns an RGBA image, diagnostics, and the labels of its label cache, and the host
encodes the image or hands it to the page.

## Building blocks

The parts the system is made of, each with one responsibility, named in the vocabulary of
[`GLOSSARY.md`](GLOSSARY.md). One level deep, deeper only where the size of a part earns it. The
golden path and the test pattern an agent copies from are named here.

- **`style-ir`** — the data model of a map, its expressions a CQL2 tree
  ([ADR-0015](adr/0015-expressions-as-cql2.md)), the diagnostics with their source spans, and the
  style document that serializes it as YAML or JSON through serde
  ([ADR-0013](adr/0013-style-document-serde.md)). A style is generic over its transform: a
  class's `Style` may leave it out, a label's `Style<LabelTransform>` always names `labelpoint`,
  `labelpoly`, or `labelcenter`, and `symbolize` and the engine draw both through functions
  generic over it. Its readers
  ([ADR-0014](adr/0014-style-document-versions.md)) name each field they do not know by its
  JSON Pointer; a document of a newer minor that does not read is read again from its value tree
  with each unknown value dropped and each condition that does not read kept as `false`, at most
  a hundred times (Q-9); a document of an older major is migrated on its value tree by the chain
  of steps in `migration.rs`, one major at a time, and then read as the current version. Its
  `schema` feature derives the JSON Schema of the current format version with schemars, checked
  in under `schema/`; `tests/schema.rs` compares a fresh derivation with that file on every push
  and, once a release tag `v<semver>` holds schema files, freezes them and asks for the bump the
  change since the last release needs. The browser bindings never enable the feature. Its
  module `c.rs` ports C's and MapServer's small functions once for the frontend, the core, and
  the adapters: glibc's `strtod`, `atof`, and `atoi`, MapServer's `MS_NINT` as `lrint` on
  x86-64 narrowed to an `int`, C's `(int)` cast of a `double`, `int_cast`, `INT_MIN` past an
  `int`, a C string's end, and `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, and
  `msStringSplit` ([ADR-0036](adr/0036-c-ports-in-style-ir.md)). `engine`, `symbolize`, and
  `label` deny clippy's `cast_possible_truncation`, so that such a cast is not written as `as`.
  `style-ir` holds the geometry of a feature (`geometry.rs`), which the ports deliver and
  `symbolize` draws, the one crate both may depend on. It holds the marker a label's
  `labelpoint` style gives, a symbol's default anchor point, and the anchor arithmetic
  (`marker.rs`): a computed value, not part of the style document, shared by `symbolize`, which
  sizes and places the marker, and `label`, which bounds it; it lives here because ADR-0008 gives
  `symbolize` no edge to `label`. It holds a map's fonts, `Fonts`, which finds a name without
  case as MapServer's font set does and leaves a style document's name that matches one written
  before it out with a warning, and the one reader of a label's font list, `font_list`,
  which the frontend checks a label's fonts with and `label` takes its steps from (`font.rs`); it
  lives here because ADR-0008 keeps `mapfile` from `label`. It depends on no other crate of the
  workspace.
- **`ports`** — the port traits — the resource loader, the feature source, the projection
  resolver — with the features a source delivers, their geometries, re-exported from `style-ir`,
  and properties
  ([ADR-0023](adr/0023-typed-properties.md)), and the resource path that
  cannot leave the base context (Q-8). Async lives here and in hosts and adapters, never `Send`;
  `poll_once` runs a future whose ports never wait in one poll, as every host and test that runs
  the frontend or the engine synchronously does.
- **`mapfile`** — the mapfile frontend: a lexer that follows MapServer's, its keywords and their
  token values in one table (`keywords.rs`), a generic parser of
  blocks and directives driven by the statement keywords of each block kind, `INCLUDE` splicing
  against the main mapfile's directory, bounded in depth, cycles, and total size (Q-9), the
  symbol file and font set `SYMBOLSET` and `FONTSET` name and the init files an `init=` names by
  a path, read through the resource loader against the same directory within the same bounds
  (`init_file.rs`, [ADR-0019](adr/0019-projection-resolver.md)), and the
  translation into the style IR (`translate/`, a module per block it reads: symbols, font set,
  labels, styles, projections, the output format's transparency, inline features, and the
  defaults of `VALIDATION`, which a load substitutes into its expressions within a 16 MiB bound of
  their own, apart from the total size (`translate/defaults.rs`, Q-9); `METADATA` and `VALIDATION`
  read by one reader, as `loadHashTable` reads them, which reports a pair it cannot read
  (`translate/hash_table.rs`); a load counts each of the two
  bounds on text by a budget of one type that stops at its bound and reports the charge that passes
  it once (`budget.rs`)), which appends
  to the map's symbols a pixmap or SVG symbol for each name a style's `SYMBOL` gives that no
  symbol carries, as `msAddImageSymbol` appends it, a name beginning `http` aside, its path
  inside the base context and its file not read
  (`translate/symbol.rs`), which reports
  every keyword it does not carry over, reads the WKT of an inline feature with a reader of its
  own, bounded in nesting (`wkt.rs`,
  [ADR-0018](adr/0018-inline-features.md)), translates
  MapServer's expressions with their typing into the CQL2 tree and a plain `TEXT`'s bindings into
  a concatenation, both read by one scanner of the expression rules of MapServer's lexer
  (`expression.rs`), and resolves
  every other `init=` through the host's projection resolver. A load reports at most a thousand
  diagnostics (Q-9). It is the only crate that knows MapServer syntax; what each parameter it
  carries means is described per parameter under `docs/semantics/`.
- **`expr`** — evaluates the style IR's expressions against a feature's properties, which it
  reads through a trait of its own, as MapServer's parser evaluates its expressions, and matches
  `regexp_like` with the `regex` crate in byte mode, each pattern a tree writes compiled once per
  render and kept, at most 1024 of them, and one computed per feature compiled where it is used
  ([ADR-0016](adr/0016-expression-evaluation.md)); C's `printf` and `strptime` it needs are
  ported in `c.rs` and `time.rs`, the reading of PCRE2's syntax in `pattern.rs`, and it reads
  numbers with `style-ir`'s `c.rs`, as the engine reads a style's or a label's bound values.
  Synchronous, without I/O; the only crate that uses `regex`.
- **`engine`** — renders a map: fits its extent to the image as MapServer does, asks each layer's
  bound feature source for the features in view, or, for a layer with inline features, a source
  in memory of its own over them, which reports a bound source as not read (`inline.rs`,
  ADR-0018), draws each feature that passes the layer's
  filter with the first class whose expression matches it, up to the layer's `MAXFEATURES`, which
  the frontend takes from the layer, its metadata, the map's, or the output format, its styles'
  bound and computed
  properties bound to it and every style and label scaled by its layer's scale factor, which
  `SIZEUNITS` or `SYMBOLSCALEDENOM` set and the map's `RESOLUTION` over its `DEFRESOLUTION`
  otherwise, the bounds of a style and a label by that resolution factor alone (`classes.rs`,
  the properties of a style a feature may bind listed once in `bindable.rs`),
  and turns each feature and style into display items
  by the layer's render mode. A layer without a source, or whose source fails, is skipped with a
  diagnostic (G-8); a source that skips records yields one warning for its layer
  ([ADR-0022](adr/0022-skipped-records.md)); the fonts a label names and the images its
  pixmap symbols name it loads by one load, once per render and path through the host's resource
  loader, each file held to the 32 MiB of Q-9 that a font file and a symbol image's file share
  (`resource.rs`, the fonts and the face a line is drawn in `fonts.rs`); each image it decodes
  once and holds within Q-9's bound on the bytes one render's images and fill tiles come to
  (`images.rs`,
  [ADR-0029](adr/0029-pixmaps-held-once-per-render.md)); it draws the labels of point
  layers after every layer, as MapServer's label cache does, through `label` (ADR-0031), and
  those of a point layer of `LABELCACHE OFF` at each point after its markers, before the next
  point's, as `pointLayerDrawShape` draws them, without a test against other labels, markers, or
  the image's edge ([label.md](semantics/label.md)): it holds each label as its glyphs' indices
  and its point, draws each glyph once per font and size, and
  builds a label's path only as it draws it; a label it cannot draw yet is left out with a
  diagnostic. It draws a label's styles before its text, as `msDrawLabelCache` does, through
  `symbolize`, which it hands the label's point and the box around its text (`labels.rs`, its
  binding to a feature `label_binding.rs`, which reads a bound value as `classes.rs` does, its
  styles held for the cache, which hand the cache their marker, and their drawing
  `label_styles.rs`); a
  label's `PRIORITY` it may compute per feature through `expr`. A label
  takes its text from the label, the class, the `LABELITEM`, or last its inline feature's own
  `TEXT`, which the port's `Feature` does not carry: the labels read it beside the port, from the
  layer's inline features in the style IR at the place the feature's id names, which holds while
  the in-memory source numbers every feature by its place (`inline.rs`, ADR-0018). It
  reports every label of its cache in `Rendered.labels`, in the order the cache walks them, with
  its member box, that of its text and its `labelpoint` marker, where it was drawn, or none where
  it was discarded. It draws each feature in its styles through
  `symbolize`, in the order MapServer's shape cache draws a line layer's styles and their outlines
  (`plan.rs`, its `Drawing` in `lib.rs`), into an image of its own for a layer with an opacity, and
  reports per layer what `symbolize` left undrawn: markers past their budget, hatches refused,
  a label's `labelpoly` hatches among them, pixmaps whose image the store or whose tile the cache
  refused, a label's among them, and geometries its render mode cannot draw (Q-9); the hatches
  and pixmaps are one value, `Refusals`, which a canvas yields, the labels keep per layer and
  merge, and one function of `lib.rs` turns into the layer's diagnostics. It loads the
  images of a layer's pixmap marks and fills, and of its labels' `labelpoint` and `labelcenter`
  pixmaps, into the render's store before it draws the layer, since `symbolize` does no I/O, and
  hands them to it on the canvas, and to the labels, which size the marker a class's first style
  holds beneath each label, and a `labelpoint` style's marker, by them. It turns every drawing
  operation into a display item of its feature in one place (`draw_items` in `lib.rs`). It is the
  one core crate that awaits the ports.
  Every host binds its sources to the layers by name through the engine's `bind_sources`
  (`bind.rs`), which reports a source no layer is named for and one that cannot be read the same
  way for each.
- **`symbolize`** — the symbolization of a style, synchronous and without I/O: a feature's geometry
  in a style bound to it turned into drawing operations, as MapServer draws it. The dispatch of a
  style by the layer's render mode, the symbols a `GEOMTRANSFORM` places, and the paths of the
  images a layer's styles will draw, which the engine loads before it draws the layer
  (`draw.rs`); a polygon's fill through MapServer's default gamma, and a polygon filled with its
  pixmap's tile (`shade.rs`); a point's marker, and a pixmap's image placed at a point, unscaled
  at a whole pixel or, scaled or turned, as the square `agg2RenderPixmapSymbol` fills about the
  point and the inverse of the image's transformation (`marker.rs`); a line's dashes, and the
  style of an outline pass, prepared once for a line and a `GEOMTRANSFORM` alike (`line.rs`); the
  canvas a layer draws on, which the images of the render's pixmaps are handed on and which
  yields what it left undrawn (`canvas.rs`); a label's styles, which of them are drawn, and their
  operations on the label's point and the box around its text: a `labelpoly` style shaded on the
  box, a `labelpoint` style's marker at the point, and a `labelcenter` style's at the centre of the
  box, a vector, an ellipse, or a pixmap drawn as a point's marker is, MapServer's default symbol
  as nothing, and a `labelpoint` style's marker sized and placed as `computeMarkerBounds` takes it,
  `label_style_marker` (`label.rs`); a style's width
  and size bounds, one function per site of MapServer's that bounds them, with the constants they
  bound by (`bounds.rs`); a property's value as bound, the gamma, and degrees to radians
  (`style.rs`); the view that maps the extent to the image, and a line's or a ring's vertices to
  the pixels msTransformShapeSimplify keeps (`extent.rs`); a style's symbol
  resolved, its vector and ellipse symbols, a marker's size and anchor point, a pixmap's scale,
  square, and transformation, in the order of operations of AGG's `trans_affine`, and the images
  of a render's pixmaps by their path (`symbol.rs`); hatches, the
  clip of a hatched polygon, and the reference point its lines count from (`hatch.rs`); vector,
  ellipse, and pixmap symbols along a line, and the layer's budget of markers
  (`line_marker.rs`); a polygon's tile, its size held to Q-9's bound, and the render's
  cache of six tiles, counted with the decoded images (`tile.rs`); line offsets (`offset.rs`); and
  the style's `GEOMTRANSFORM` and `POLAROFFSET` (`geom_transform.rs`). A line style's symbol with a
  `GAP` it places along the line; those markers, a point layer's at its features, and the ones a
  `GEOMTRANSFORM` places share one budget per layer, `MAX_MARKER_POINTS`, each symbol priced and
  taken from it one way (Q-9); the square Forager draws at a point for symbol 0, which MapServer
  does not draw, takes none of it. Its modules import one another in one direction only, `draw.rs`
  and `label.rs` at the top, then `shade.rs`, `marker.rs`, `line.rs`, `canvas.rs`, `hatch.rs`,
  `line_marker.rs` and `tile.rs`, `symbol.rs`, and `geom_transform.rs`, and `bounds.rs`, `style.rs`,
  `extent.rs`, and `offset.rs` the leaves, as `lib.rs` lists them; ADR-0008
  decides the direction only between crates, and `arch-test`'s `tests/symbolize_modules.rs` holds
  this one.
- **`label`** — a label's text as MapServer lays out its Latin lines: its sizes, the fonts of its
  font list, read by `style-ir`'s `font_list`, in the order MapServer tries them, and its glyphs
  read with skrifa, unhinted with FreeType's arithmetic, laid out by their advances, broken into
  lines at the label's `WRAP` character and its `MAXLENGTH`, aligned line by line, and turned by the
  label's angle; and where a label stands, its point and its box placed by its position, its offset,
  and its marker's, with its labelpoly and the bounds of its `labelpoint` marker, as a pure function
  of the text's box (`place.rs`); and which labels MapServer's label cache draws, by priority,
  collisions with the labels drawn, their `labelpoint` markers among them, and the markers cached,
  the image's edge and the gutter inside it that `labelcache_map_edge_buffer` sets, and
  `MINDISTANCE`, each test bounded by a grid over the image that files a drawn label by its box and,
  for `MINDISTANCE`, by its point apart; the cache takes the first `MAX_LABELS`, reports how many it
  left out, and hands its caller every label it visits, drawn or not, in its walk's order, with the
  box it filed a drawn one by, which the engine's report of the labels follows; it computes the box
  of the marker cached beneath a label and `msDrawLabelCache`'s offset of a label from its markers
  (`cache.rs`). It is the one crate that depends on skrifa, and does no I/O (ADR-0031).
- **`render`** — the display list, its fills and hatches each with the gamma the engine gives them
  or none, a pixmap's image placed at a whole pixel or resampled into a square through an affine
  transformation, each by a counted reference to the one image, a polygon filled with a tile, by
  a counted reference to the tile, which holds its size within Q-9's bound, the drawing that makes
  it, and its image once the renderer has built it (`tile.rs`), the
  renderer trait, the RGBA image, and the premultiplied image a pixmap is decoded to
  (`PremultipliedImage`, ADR-0029); the cost model that bounds a hatch before it is cut
  (`Hatch::is_bounded`, ADR-0028, `hatch.rs`); and the clip of every path to the image, its
  segments, lines, and rings, the image grown by its margin, the width a stroke is drawn at, and the
  reach past the image a glyph's point may have (`MAX_GLYPH_REACH`), properties of the display list
  in one module (`clip.rs`, [ADR-0027](adr/0027-clip-to-the-image.md)),
  which the bound measures a hatch's lines with and every renderer clips with.
- **`render-agg`** — the reference renderer on agg-rust; it clips every path near the image
  before rasterizing, by the clip of `render`, fills through the gamma it is given,
  blends premultiplied colours
  in a pixel format of its own as MapServer's AGG 2.4 does, draws a layer with an opacity into an
  image of its own and lays it over, fills glyphs as MapServer's AGG renderer does, blends a
  pixmap's image as `blend_from` does, resamples a scaled or turned one with a port of AGG 2.4's
  bilinear clip filter and linear interpolator, `iround` being `style-ir`'s `lrint` narrowed to an
  `int`, over the packed scanlines of the square it fills (`resample.rs`,
  [ADR-0026](adr/0026-agg-blender-and-resampler.md)), builds a tile the first time
  it fills with it, in a transparent image of its own, and repeats it from the image's origin, and
  is the one crate that may use `unsafe` ([ADR-0025](adr/0025-agg-rust-rasterizer.md)); it cuts a
  hatch from its polygon with i_overlay, the one crate that depends on it (ADR-0028).
- **`encode`** — PNG from and to an RGBA image, and a pixmap symbol's PNG decoded as MapServer's
  `readPNG` decodes it, premultiplied, its header held to Q-9's 4096 × 4096 before anything is
  allocated for it (`pixmap.rs`, [ADR-0030](adr/0030-png-encoder.md),
  [ADR-0029](adr/0029-pixmaps-held-once-per-render.md)).
- **`adapter-geojson`** — a feature source over a GeoJSON document
  ([ADR-0020](adr/0020-geojson-adapter.md)).
- **`adapter-shapefile`** — a feature source over an ESRI shapefile, its `.shp`, `.shx`, and
  `.dbf` loaded through the resource loader and read with the records MapServer reads, in memory
  that follows the files' size ([ADR-0021](adr/0021-shapefile-adapter.md)), each rule of the
  reading written down in `docs/semantics/shapefile.md`; a record it cannot read is skipped and
  counted, and the engine reports it per layer
  ([ADR-0022](adr/0022-skipped-records.md)).
- **`adapter-epsg`** — resolves `init=epsg:` projections for the frontend through the
  `ProjectionResolver` port: built in are only EPSG:4326, EPSG:3857, and the WGS84 UTM zones;
  every other code comes from a table the host supplies
  ([ADR-0019](adr/0019-projection-resolver.md)).
- **`cli`** — the `forager` command, a host with a file loader rooted at the base context — the
  mapfile's directory, or the directory `--base` names — that also refuses symbolic links out of
  it, and that hands the frontend an `adapter-epsg` table, extended by the file `--epsg` names;
  `forager render` binds each local layer the engine draws to the shapefile its `DATA` names, and
  the GeoJSON files `--source` names to layers by name, and writes a PNG. A file is checked by its
  path before it is opened, so one who can write into the base context during a load can swap it in
  the window between, for a link out of the base or for a FIFO; `read_bounded` in `src/files.rs`
  names the risk the maintainer accepted. Its binding of a map's sources and fonts to a base
  context is the crate's library (`src/lib.rs`), on which the binary is built and through which
  the conformance harness renders.
- **`bindings-wasm`** — the host for the browser: a mapfile, GeoJSON per layer name, an optional
  EPSG table, a viewport, and the files the mapfile names, such as its font set and fonts, in, the
  RGBA image and the diagnostics out; it keeps a layer's source reference only when that is a
  `DATA` without credentials, and drops and unbinds every other (Q-7).
  `scripts/build-web.sh` builds it into `target/web/`. Its feature `conformance` adds exports of
  plain integers that render a style document with inline features and the files of a render, for
  the comparison of G-9; `scripts/check-rust.sh` builds that module into `target/conformance/`
  ([ADR-0034](adr/0034-wasm-renders-compared-in-wasmi.md)).
- **`dev-server`** — serves `examples/web/` and the bundle on localhost, for trying it out. The
  page shows the image, and a Leaflet map whose tiles each call the bindings with the tile's
  extent, grown by a buffer of a tile on every side that it cuts away, so that the label cache
  places a label beside a tile's edge as the neighbouring tile does; its labels use the font
  DejaVu Sans, which the page hands in beside the mapfile. Leaflet and the font are vendored
  copies that `scripts/check-vendor.sh` checks by checksum
  ([ADR-0035](adr/0035-vendored-third-party-files.md)).
  The sample's GeoJSON is a synthetic landscape of 4000 × 2500 map units that
  `scripts/generate-example.sh` writes from a fixed seed in bash and awk;
  `scripts/test-generate-example.sh` holds two runs of it to each other and to the checked-in
  files.
- **`arch-test`** — the dependency test of [ADR-0008](adr/0008-ports-and-adapters-crates.md):
  the allowed edges between the crates, and no C code under the IR, the core, the ports, or the
  frontend; it also refuses `unsafe` outside `render-agg`
  ([ADR-0025](adr/0025-agg-rust-rasterizer.md)), skrifa outside `label` (ADR-0031), `regex`
  outside `expr` ([ADR-0016](adr/0016-expression-evaluation.md)), and i_overlay outside
  `render-agg` (ADR-0028), `adapter-shapefile` as a dependency of `bindings-wasm`
  ([ADR-0024](adr/0024-browser-bindings-bind-geojson-only.md)), `wasmi` anywhere but among the
  dev-dependencies of `cli` ([ADR-0034](adr/0034-wasm-renders-compared-in-wasmi.md)), and a
  future polled with a no-op waker anywhere but `poll_once` of `ports`, and requires every crate
  on the output path, the IR, the core, the ports, the frontend, and the adapters, to turn on the
  lints of `clippy.toml` that ban hash maps, clocks, and random sources (Q-2) and to lift them
  nowhere but the one test time bound it names; the hosts alone are free. `tests/c_numbers.rs`
  refuses a definition of one of `c.rs`'s ports, or one named `lrint_…` or `ms_nint_…`, outside
  `style-ir`'s `c.rs`, and a `PI / 180` in code (ADR-0036).
  `tests/symbolize_modules.rs` refuses an import between the modules of `symbolize`, outside its
  tests, against the direction its `lib.rs` states, and a module of the crate its table has no row
  for. `tests/symbol_names.rs` refuses an `eq_ignore_ascii_case` near a symbol's name outside
  `symbolize`'s `symbol.rs`, whose `named_index` the engine's binding of a `SYMBOL` calls; the
  frontend keeps a lookup of its own in another form, which ADR-0008 keeps from `symbolize`.

Outside the workspace, `fuzz/` is a Cargo workspace of its own with the fuzz targets of Q-3 — the
mapfile frontend, the style document readers with and without a migration step, the renderer on
structured shapes with a label of arbitrary text and size and its pixmap markers, a pixmap at
points, along lines, and tiled over polygons, the layers' feature limit and drawing after the label
cache, the cache's edge buffer, and expressions built from the same bytes, the expression
evaluator on arbitrary trees, patterns, and properties, the browser path on GeoJSON, the shapefile
adapter on arbitrary `.shp`, `.shx`, and `.dbf` files, the text path on an arbitrary font file, and
the pixmap decoder on an arbitrary image file — run by `scripts/fuzz.sh`, which reads them from the
manifest, every Sunday through `.github/workflows/fuzz.yml`; it keeps libFuzzer and the nightly
toolchain out of the product's build, and `scripts/check-rust.sh` lints it on every push. Beside it,
`scripts/mutants.sh` runs cargo-mutants over the workspace every week through
`.github/workflows/mutants.yml` and reports the edits no test notices. `scripts/package.sh` packs
the CLI's archive and the browser bindings' package under `target/package/`, each with
`THIRD-PARTY-NOTICES.md` and the two licences, and reads each archive back for them, by the check
of the notices that `scripts/check-notice.sh` shares (`scripts/lib/notice.sh`); nothing is
published.

The conformance suite is msautotest at the MapServer release that `scripts/fetch-msautotest.sh`
pins, by tag and commit: a sparse checkout into `target/` of its mapfiles, the shapefiles, fonts,
and symbol images the cases of `misc` and `renderers` read, and the non-cairo PNG references of
those two directories, cached in CI, never in the repository. `scripts/check-rust.sh` loads every
mapfile of it through the CLI on every push (G-1), with the suite's directory as the base context
and a stand-in table for the EPSG codes beyond the built-in ones, and round-trips the style IR of
each through YAML and JSON (G-3); `crates/cli/tests/msautotest.rs` names the files it skips with a
reason each, and loads the one mapfile that names the suite's init file bare in-process with a
stand-in for it. Its module `metric/` compares a rendering with a reference image as RGBA buffers,
decoding the reference by the `png` crate whatever its colour type, and writes the rendering and a
difference image when a case fails. Beside them, `.github/workflows/noise.yml` measures MapServer's
own noise against the same references: `scripts/build-mapserver.sh` builds `map2img` from the pinned
commit on the runner ([ADR-0033](adr/0033-noise-measurement.md)), and the harness's module
`conformance/noise/` runs it over the cases in scope and reports the share of differing pixels per
case at each threshold of a set, and its maximum over each category. Its ignored test runs where
`FORAGER_MAP2IMG` and `FORAGER_MAPSERVER_LABELS` name MapServer's build and skips itself where
neither is set, so `scripts/check-rust.sh` runs it without naming it and it measures nothing there;
the workflow runs the ignored tests of that module and fails when its job summary holds no report.
The conformance harness there reads the cases of `misc` and `renderers`, their
`RUN_PARMS` lines, in its module `conformance/`, whose list `cases.txt` names each in scope with its
category and expected outcome, passing or failing, or out of scope with its reason, and marks those
whose reference is blank; it renders each case in scope in-process: with the suite's directory as
the base context, the mapfile loads through the CLI's `FileLoader`, as `forager render --base`
loads it, with the stand-in EPSG table and the case's `-i` applied, and the CLI's library binds its
shapefiles and reads the fonts of its labels from the same `FileLoader` as `forager render` does;
G-1's in-process load and G-3's round trips load through it too; it compares the rendering by the
metric of its category, geometry and symbols or text, and holds its outcome, leaving the renderings
of failing cases under Cargo's `target/tmp/msautotest/`. It counts the cases in scope and those that
pass, the blank ones apart, and `scripts/check-fidelity.sh` holds the share of passing cases
without a blank reference against that of the last release tag `v<semver>` in the record
`crates/cli/tests/conformance/fidelity.txt`, to which each release adds its line (Q-4); no release
is recorded yet, and until one is the check passes. For each text case it compares the labels the
engine reports in its `Rendered` with MapServer's, recorded under `conformance/labels/`, and holds
the list's mark `labels-hold` of the cases whose labels all match (G-10). A test that runs on every
push without the suite holds the records under `conformance/labels/` and the text cases in scope of
the list equal, naming each record no such case reads and each such case without its record. The
noise measurement runs, for each text case, the C reader `conformance/mapserver_labels.c` beside
`map2img`, which `scripts/build-mapserver.sh` compiles against the pinned build (ADR-0033); it holds
the reader's image byte for byte equal to `map2img`'s and fails when the labels the reader takes
from MapServer's label cache differ from the record, and keeps them in the run's artifact, from
which a record is taken again.

The same harness holds G-9 (ADR-0034): its module `conformance/wasm.rs` writes the features of each
case's shapefiles into its map, renders the style document natively and in the bindings' wasm32
module, which wasmi runs, with the files the native render read, and compares the two RGBA buffers
byte for byte, naming the case and the first pixel that differs; a case whose map does not load or
whose shapefile does not read is a difference too. The native render and the module's input format
are written in the test and again in `crates/bindings-wasm/src/conformance.rs`.

The crates and their allowed edges are [ADR-0008](adr/0008-ports-and-adapters-crates.md); the
toolchain and its checks are [ADR-0007](adr/0007-rust-toolchain.md).

**Golden path:** `forager read`, from the CLI's argument through the file loader and the
frontend's `load` to the style document — `crates/cli/src/main.rs`, `crates/cli/src/files.rs`,
`crates/mapfile/src/lib.rs`.
**Test pattern:** behaviour is tested at the highest seam that decides it — the frontend's `load`
with files in memory (`crates/mapfile/src/tests.rs`, and beside each module of the translation
that it tests), the binary as a process with fixtures (`crates/cli/tests/read.rs`) — each test
naming the goal or ADR it verifies with `Verifies:`.

## How it runs

The few paths worth following end to end, such as a request, a job, or a build, and where state
lives between them. Only what a newcomer would otherwise reconstruct from code.

`forager read <mapfile>`: the CLI canonicalizes the mapfile's path and roots a file loader at the
base context, its directory or the one `--base` names. The frontend reads the mapfile through the
loader, lexes it, and splices in each `INCLUDE`'s tokens, resolved against that directory as
MapServer resolves them. The parser builds the `MAP` block; the symbol file, the font set, and the
init files it names are read next, through the same loader and within the same size bound. The
translation then walks the block into the style IR, resolving each `init=` from an init file or
through the CLI's EPSG table, and adds a warning for each keyword not carried over. With no error,
the CLI prints the style document; diagnostics go to standard error. No state outlives the call.

`forager render <mapfile> --source <layer>=<file> -o <png>` loads the mapfile the same way and hands
it to the binding of the CLI's library (`render` in `src/lib.rs`), which binds each local layer the
engine draws and whose shapefile the base context holds to the shapefile adapter, opening each
shapefile once for all the layers that name it, parses the GeoJSON file of each name a layer carries
into a source bound to that layer instead, reads the fonts its labels name from the base context,
and runs the engine with the agg-rust renderer. For each layer that is ON or DEFAULT, in order, the
engine queries the source with the fitted extent and draws its features in the styles of the first
class whose expression matches: a point or polygon layer each feature in every style before the next
feature, a point layer's point by point, every style's marker at a point before the next point,
whatever its `LABELCACHE`, and a line layer style by style over all its features, with each
style's outline in a pass before the style, as MapServer's line layers do. The labels of point
layers are placed by the label cache and drawn after the last layer, but for those of a point
layer of `LABELCACHE OFF`, drawn after each of its features; a label of a line or a polygon layer
is not drawn yet. A layer of `POSTLABELCACHE TRUE` is drawn after the label cache, past every
other layer, its labels at once. The image is encoded as PNG.

In the browser, the page calls the bundle's `render`, or `renderWith`, with the mapfile text, a JSON
object of GeoJSON by layer name, an optional viewport, and the files the mapfile names. The bindings
hand the frontend a loader that holds the mapfile and the files the page handed in, and nothing
else; they drop every source reference the credential filter does not admit, apply the viewport over
`SIZE` and `EXTENT`, parse the GeoJSON, and run the engine with the agg-rust renderer. Every port
answers from memory, so each future is polled once; one that waits ends the call with a diagnostic
instead of a spinning tab. The page puts the pixels on a canvas and lists the diagnostics.
