# Architecture — Forager

> The system **as it stands today**: the parts it is built from, what each is responsible for, and
> how they fit together. It holds no rule and no decision. It names the ADR behind each structural
> choice; where it disagrees with an accepted ADR, the ADR is right and this document is stale. It
> is updated in the same change as the structure it describes
> ([ADR-0001](adr/0001-agent-governance-model.md)).
>
> **Last design revision:** 2026-10-25, due after 20 changes. The revision that ran moves the
> date; the number is this project's to set; a sensor counts the changes outside `docs/` since
> the commit that set the date and says when the next is due
> ([ADR-0006](adr/0006-feature-layer.md)).

## Context

What sits outside the system and what crosses its boundary. A diagram earns its place here more
than anywhere else, and none beats one that has stopped being true.

A host embeds the core and owns all I/O: the CLI reads files, and the browser bindings take what a
page passes in. The host hands the mapfile frontend a resource loader rooted at the base context
and a projection resolver; the frontend returns a style document and diagnostics. To render, the
host binds a feature source to each layer by the layer's name and hands the engine a renderer;
the engine returns an RGBA image and diagnostics, and the host encodes it or hands it to the
page.

## Building blocks

The parts the system is made of, each with one responsibility, named in the vocabulary of
[`GLOSSARY.md`](GLOSSARY.md). One level deep, deeper only where the size of a part earns it. The
golden path and the test pattern an agent copies from are named here.

- **`style-ir`** — the data model of a map, its expressions a CQL2 tree
  ([ADR-0030](adr/0030-cql2-expressions.md)), the diagnostics with their source spans, and the
  style document that serializes it as YAML or JSON through serde
  ([ADR-0014](adr/0014-style-document-serde.md)). Its readers
  ([ADR-0020](adr/0020-style-document-compatibility.md)) name each field they do not know by its
  JSON Pointer; a document of a newer minor that does not read is read again from its value tree
  with each unknown value dropped and each condition that does not read kept as `false`, at most
  a hundred times (Q-9); a document of an older major is migrated on its value tree by the chain
  of steps in `migration.rs`, one major at a time, and then read as the current version. Its
  `schema` feature derives the JSON Schema of the current format version with schemars, checked
  in under `schema/`; `tests/schema.rs` compares a fresh derivation with that file on every push
  and, once a release tag `v<semver>` holds schema files, freezes them and asks for the bump the
  change since the last release needs. The browser bindings never enable the feature. It
  depends on no other crate of the workspace.
- **`ports`** — the port traits — the resource loader, the feature source, the projection
  resolver — with the features a source delivers, their geometries and properties
  ([ADR-0026](adr/0026-feature-properties-at-the-port.md)), and the resource path that
  cannot leave the base context (Q-8). Async lives here and in hosts and adapters, never `Send`.
- **`mapfile`** — the mapfile frontend: a lexer that follows MapServer's, a generic parser of
  blocks and directives driven by the statement keywords of each block kind, `INCLUDE` splicing
  against the main mapfile's directory, bounded in depth, cycles, and total size (Q-9), the
  symbol file and font set `SYMBOLSET` and `FONTSET` name and the init files an `init=` names by
  a path, read through the resource loader against the same directory within the same bounds
  (`init_file.rs`, [ADR-0023](adr/0023-projection-resolver-and-init-files.md)), and the
  translation into the style IR (`translate/`, a module per block it reads: symbols, font set,
  labels, styles, projections), which reports every keyword it does not carry over, translates
  MapServer's expressions with their typing into the CQL2 tree (`expression.rs`), and resolves
  every other `init=` through the host's projection resolver. A load reports at most a thousand
  diagnostics (Q-9). It is the only crate that knows MapServer syntax; what each parameter it
  carries means is described per parameter under `docs/semantics/`.
- **`expr`** — evaluates the style IR's expressions against a feature's properties, which it
  reads through a trait of its own, as MapServer's parser evaluates its expressions, and matches
  `regexp_like` with the `regex` crate in byte mode, each pattern compiled once per render
  ([ADR-0031](adr/0031-expression-evaluator.md)); C's `atof`, `printf`, and `strptime` it needs
  are ported in `c.rs` and `time.rs`, the reading of PCRE2's syntax in `pattern.rs`. Synchronous,
  without I/O; the only crate that uses `regex`.
- **`engine`** — renders a map: fits its extent to the image as MapServer does, asks each layer's
  bound feature source for the features in view, draws each feature that passes the layer's
  filter with the first class whose expression matches it, its styles' bound and computed
  properties bound to it (`classes.rs`), and turns each feature and style into display items
  by the layer's render mode. A layer without a source, or whose source fails, is skipped with a
  diagnostic (G-8); a source that skips records yields one warning for its layer
  ([ADR-0025](adr/0025-source-reports-skipped-records.md)); the fonts a label names it loads once
  per render through the host's resource loader, each bounded, and it draws the labels of point
  layers after every layer, as MapServer's label cache does, through `label` (ADR-0028): it holds
  each label as its glyphs' indices and its point, draws each glyph once per font and size, and
  builds a label's path only as it draws it; a label it cannot draw yet is left out with a
  diagnostic. It is the one core crate
  that awaits the ports. Every host binds its sources to the layers by name through the engine's
  `bind_sources`, which reports a source no layer is named for and one that cannot be read the same
  way for each. A line style's symbol with a `GAP` it places along the line, the markers of one
  layer bounded with a diagnostic (Q-9).
- **`label`** — a label's text as MapServer lays out a Latin line: its sizes, the fonts of its
  font list in the order MapServer tries them, and its glyphs read with skrifa, unhinted with
  FreeType's arithmetic, laid out by their advances and centred on the label's point; it is the
  one crate that depends on skrifa, and does no I/O (ADR-0028).
- **`render`** — the display list, the renderer trait, and the RGBA image.
- **`render-agg`** — the reference renderer on agg-rust; it clips every path near the image
  before rasterizing, blends premultiplied colours in a pixel format of its own as MapServer's
  AGG 2.4 does, draws a layer with an opacity into an image of its own and lays it over, fills
  glyphs as MapServer's AGG renderer does, and is the one crate that may
  use `unsafe`
  ([ADR-0015](adr/0015-agg-rust-rasterizer.md)); it cuts a hatch from its polygon with
  i_overlay, the one crate that depends on it (ADR-0032).
- **`encode`** — PNG from and to an RGBA image ([ADR-0017](adr/0017-png-crate.md)).
- **`adapter-geojson`** — a feature source over a GeoJSON document
  ([ADR-0016](adr/0016-geojson-crate.md)).
- **`adapter-shapefile`** — a feature source over an ESRI shapefile, its `.shp`, `.shx`, and
  `.dbf` loaded through the resource loader and read with the records MapServer reads, in memory
  that follows the files' size ([ADR-0024](adr/0024-shapefile-reader.md)), each rule of the
  reading written down in `docs/semantics/shapefile.md`; a record it cannot read is skipped and
  counted, and the engine reports it per layer
  ([ADR-0025](adr/0025-source-reports-skipped-records.md)).
- **`adapter-epsg`** — resolves `init=epsg:` projections for the frontend through the
  `ProjectionResolver` port: built in are only EPSG:4326, EPSG:3857, and the WGS84 UTM zones;
  every other code comes from a table the host supplies
  ([ADR-0023](adr/0023-projection-resolver-and-init-files.md)).
- **`cli`** — the `forager` command, a host with a file loader rooted at the base context — the
  mapfile's directory, or the directory `--base` names — that also refuses symbolic links out of
  it, and that hands the frontend an `adapter-epsg` table, extended by the file `--epsg` names;
  `forager render` binds GeoJSON files to layers and writes a PNG.
- **`bindings-wasm`** — the host for the browser: a mapfile, GeoJSON per layer name, an optional
  EPSG table, a viewport, and the files the mapfile names, such as its font set and fonts, in, the
  RGBA image and the diagnostics out; it keeps a layer's source reference only when that is a
  `DATA` without credentials, and drops and unbinds every other (Q-7).
  `scripts/build-web.sh` builds it into `target/web/`.
- **`dev-server`** — serves `examples/web/` and the bundle on localhost, for trying it out. The
  page shows the image, and a Leaflet map whose tiles each call the bindings with the tile's
  extent; Leaflet is a vendored copy that `scripts/check-vendor.sh` checks by checksum
  ([ADR-0021](adr/0021-vendored-leaflet.md)).
- **`arch-test`** — the dependency test of [ADR-0009](adr/0009-ports-and-adapters-crates.md):
  the allowed edges between the crates, and no C code under the IR, the core, the ports, or the
  frontend; it also refuses `unsafe` outside `render-agg`
  ([ADR-0015](adr/0015-agg-rust-rasterizer.md)), skrifa outside `label` (ADR-0028), `regex`
  outside `expr` ([ADR-0031](adr/0031-expression-evaluator.md)), and i_overlay outside
  `render-agg` (ADR-0032), and requires every crate of the IR and the core to turn on the lints of
  `clippy.toml` that ban hash maps, clocks, and random sources (Q-2) and to lift them nowhere but
  the one test time bound it names; the frontend, the ports, the adapters, and the hosts are not
  held to the ban.

Outside the workspace, `fuzz/` is a Cargo workspace of its own with the fuzz targets of Q-3 — the
mapfile frontend, the style document readers with and without a migration step, the renderer on
structured shapes with a label of arbitrary text and size and with expressions built from the same
bytes, the expression evaluator on arbitrary trees, patterns, and properties, the browser path on
GeoJSON, the shapefile adapter on arbitrary `.shp`, `.shx`, and `.dbf` files, and the text path on
an arbitrary font file — run by `scripts/fuzz.sh`, which reads them from the manifest, every
Sunday through `.github/workflows/fuzz.yml`; it keeps libFuzzer and the nightly toolchain out of
the product's build, and `scripts/check-rust.sh` lints it on every push. Beside it,
`scripts/mutants.sh` runs cargo-mutants over the workspace every week through
`.github/workflows/mutants.yml` and reports the edits no test notices.

The conformance suite is msautotest at the MapServer release that `scripts/fetch-msautotest.sh`
pins, by tag and commit: a sparse checkout into `target/` of its mapfiles, the shapefiles, fonts,
and symbol images the cases of `misc` and `renderers` read, and the non-cairo PNG references of
those two directories, cached in CI, never in the repository.
`scripts/check-rust.sh` loads every mapfile of it through the CLI on every push (G-1), with the
suite's directory as the base context and a stand-in table for the EPSG codes beyond the built-in
ones, and round-trips the style IR of each through YAML and JSON (G-3);
`crates/cli/tests/msautotest.rs` names the files it skips with a reason each, and loads the one
mapfile that names the suite's init file bare in-process with a stand-in for it. Its module
`metric/` compares a rendering with a reference image as RGBA buffers, decoding the reference by
the `png` crate whatever its colour type, and writes the rendering and a difference image when
a case fails.
Beside them, `.github/workflows/noise.yml` measures MapServer's own noise against the same
references: `scripts/build-mapserver.sh` builds `map2img` from the pinned commit on the runner
([ADR-0027](adr/0027-mapserver-built-from-the-pin.md)), and the harness's ignored test
`mapserver_renders_the_cases_in_scope_within_its_noise` runs it over the cases in scope and
reports the share of differing pixels per case at each threshold of a set.
The conformance harness there reads the cases of `misc` and `renderers`, their `RUN_PARMS` lines,
in its module `conformance/`, whose list `cases.txt` names each in scope with its category and
expected outcome, passing or failing, or out of scope with its reason, and marks those whose
reference is blank; it renders each case in scope in-process, binding shapefiles as
`forager render` does, and holds its outcome, leaving the renderings of failing cases under
Cargo's `target/tmp/msautotest/`. It counts the cases in scope and those that pass, the blank
ones apart, and `scripts/check-fidelity.sh` holds the share of passing cases without a blank
reference against that of the last release tag `v<semver>` in the record
`crates/cli/tests/conformance/fidelity.txt`, to which each release adds its line (Q-4).

The crates and their allowed edges are [ADR-0009](adr/0009-ports-and-adapters-crates.md); the
toolchain and its checks are [ADR-0008](adr/0008-rust-toolchain.md).

**Golden path:** `forager read`, from the CLI's argument through the file loader and the
frontend's `load` to the style document — `crates/cli/src/main.rs`, `crates/mapfile/src/lib.rs`.
**Test pattern:** behaviour is tested at the highest seam that decides it — the frontend's `load`
with files in memory (`crates/mapfile/src/tests.rs`, and beside each module of the translation
that it tests), the binary as a process with fixtures (`crates/cli/tests/read.rs`) — each test
naming the goal or ADR it verifies with `Verifies:`.

## How it runs

The few paths worth following end to end, such as a request, a job, or a build, and where state
lives between them. Only what a newcomer would otherwise reconstruct from code.

`forager read <mapfile>`: the CLI canonicalizes the mapfile's path and roots a file loader at its
directory. The frontend reads the mapfile through the loader, lexes it, and splices in each
`INCLUDE`'s tokens, resolved against that directory as MapServer resolves them. The parser builds
the `MAP` block; the symbol file, the font set, and the init files it names are read next, through
the same loader and within the same size bound. The translation then walks the block into the
style IR, resolving each `init=` from an init file or through the CLI's EPSG table, and adds a
warning for each keyword not carried over. With no error, the CLI prints the style document;
diagnostics go to standard error. No state outlives the call.

`forager render <mapfile> --source <layer>=<file> -o <png>` loads the mapfile the same way, binds
each local layer the engine draws and whose shapefile the base context holds to the shapefile
adapter, opening each shapefile once for all the layers that name it, parses the
GeoJSON file of each name a layer carries into a source bound to that layer instead, and runs the
engine with the agg-rust renderer.
For each layer that is ON or DEFAULT, in order, the engine queries the source with the fitted
extent and draws its features in the styles of the first class: a point or polygon layer each
feature in every style before the next feature, a line layer style by style over all its
features, with each style's outline in a pass before the style, as MapServer's line layers do.
The labels of point layers are drawn after the last layer. The image is encoded as PNG.

In the browser, the page calls the bundle's `render`, or `renderWith`, with the mapfile text, a JSON
object of GeoJSON by layer name, an optional viewport, and the files the mapfile names. The bindings
hand the frontend a loader that holds the mapfile and the files the page handed in, and nothing
else; they drop every source reference the credential filter does not admit, apply the viewport over
`SIZE` and `EXTENT`, parse the GeoJSON, and run the engine with the agg-rust renderer. Every port
answers from memory, so each future is polled once; one that waits ends the call with a diagnostic
instead of a spinning tab. The page puts the pixels on a canvas and lists the diagnostics.
