# Architecture Decision Records

This directory contains all Architecture Decision Records (ADRs) for this project. Accepted ADRs are
**binding** for humans and coding agents alike (see [`AGENTS.md`](../../AGENTS.md) in the repository
root). ADRs derive from the specification in [`docs/SPECIFICATION.md`](../SPECIFICATION.md).

## Mechanics

The rules live in three `NUC-` ADRs.
[NUC-decisions-are-proposed-then-accepted](NUC-decisions-are-proposed-then-accepted.md) decides
when an ADR is written, how it is developed, who flips its status, and that an accepted one is
immutable. [NUC-adrs-are-named-by-category](NUC-adrs-are-named-by-category.md) decides how it is
named, cited, superseded, and indexed. [NUC-base-is-fixed](NUC-base-is-fixed.md) decides the fixed
base. This section carries only the mechanics of the record;
[`scripts/check-docs.sh`](../../scripts/check-docs.sh) names in its header which of them it
verifies.

- **File.** Copy [`template.md`](template.md) to `<CAT>-<slug>.md` and head it
  `# <CAT>-<slug>: <title>`, the category from the [list](#categories). The author picks the
  identifier when creating the file, and it never changes or returns
  ([NUC-adrs-are-named-by-category §1](NUC-adrs-are-named-by-category.md)).
- **Wording.** The *Decision* and the *Enforcement* follow the writing rule of
  [`AGENTS.md` §4](../../AGENTS.md#4-working-style).
- **Index.** The tables below list the project's ADRs, grouped by category, and no `NUC-` ADR
  ([NUC-adrs-are-named-by-category §5](NUC-adrs-are-named-by-category.md)). They change in the same
  commit as the ADR they describe, for additions, supersessions, and status flips alike. The status
  is shown via the legend's bullet and the ADR's `Applies to` header is mirrored verbatim in its own
  column. The index routes a reader from a change to the decisions that bind it, so a row says what
  its ADR governs without the file being opened. Two tables keep what a session reads apart from
  what it looks up: an accepted or proposed ADR sits under **Binding**, a superseded or rejected one
  under **Superseded and rejected**, and a status flip moves the row.
- **Identifiers are permanent once on `main`.** A normalization keeps them, and only a merged or
  split ADR takes a new one
  ([NUC-decisions-are-proposed-then-accepted §8](NUC-decisions-are-proposed-then-accepted.md)).
  Between two normalizations, never rename, delete, or merge an ADR there: other ADRs, commits
  (`Implements <CAT>-<slug>`), and code cite it. A superseded ADR keeps its file and its body. The
  superseding ADR's `Supersedes` field names it from the proposal on; its `Status` line flips to
  `⚪ superseded by <CAT>-<slug>` in the change where a human accepts the successor
  ([NUC-adrs-are-named-by-category §4](NUC-adrs-are-named-by-category.md)), its index row moving
  with it.
- **Between normalizations, sprawl is curbed by supersession, never by editing.** One ADR may
  supersede several whose decisions have grown into one. Its `Supersedes` field names each, and
  each flips its status when the superseding ADR is accepted, so the set an agent reads shrinks
  while every identifier and every body stays.
- **A normalization restates the record**
  ([NUC-decisions-are-proposed-then-accepted §8](NUC-decisions-are-proposed-then-accepted.md)).
  - **Start.** The tree has no proposed ADR, no feature spec or ticket under `docs/plan/`, and
    green checks.
  - **Steps.** Three, in this order on one branch, each handed to the human and confirmed
    before the next begins: the restated specification, whose `G-n` and `Q-n` run from 1; the
    supersessions it calls for; and the restated record. The record begins once the human has
    decided each supersession. One pull request lands them, merged by a human.
  - **Identifiers.** An ADR restated alone keeps its identifier; a merged or split one takes a
    new one. The `NUC-` files stay as the release brought them.
  - **Clauses.** Every clause of an old ADR lives on in one new ADR. The hand-off of the
    record lists each clause with the new ADR that carries it.
  - **Header.** A new ADR keeps the status of the ADRs it restates. Its `Deciders` name all of
    theirs, its `Date` is the latest of theirs, and it has no `Supersedes` field. In an
    adoption, an ADR the project had accepted stays accepted. A decision found outside any
    record enters as proposed.
  - **Dead ADRs.** Superseded and rejected ADRs are deleted. What one of them chose, and why it
    did not stand, becomes an entry under *Alternatives considered* in the new ADR that governs
    its topic.
  - **References.** Each step moves its references with a map, old to new, applied by the map
    script of the `normalize` skill. The map stands in the hand-off and the
    commit body, and in no document. A commit older than the normalization cites the identifiers
    it had then.
- **Cite only what exists.** Every citation, `<CAT>-<slug>` or a clause `<CAT>-<slug> §n`, names a
  file already in this directory. An anticipated follow-up is described by topic ("a follow-up ADR
  on session storage"). What counts as a citation, and its form in Markdown, is
  [NUC-adrs-are-named-by-category §6](NUC-adrs-are-named-by-category.md).

## Index

**The base:** the template's `NUC-` ADRs in this directory, each named with its blob in
[`NUC.blobs`](NUC.blobs), as `AGENTS.md` is. They bind like any ADR in force, and a project
extends or narrows a clause in an ADR of its own, never edits one
([NUC-base-is-fixed §1](NUC-base-is-fixed.md)).

### Categories

The category of an identifier says what its decision is about
([NUC-adrs-are-named-by-category §2](NUC-adrs-are-named-by-category.md)). The fixed ones:

- **NUC** — the template's own ADRs, reserved to the template repository.
- **SEC** — security.
- **OPS** — build, release, deployment, and operation.
- **API** — interfaces the system offers and consumes, adapters among them.

The project's own follow here, each in the same form with one line on what it holds:

- **ARCH** — the shape of the code: its crates, the modules they share, and the guards around
  third-party crates.
- **IR** — the style IR: the style document's format and versions, its expressions and their
  evaluation, and its geometry transforms.
- **REN** — rendering: rasterizing, labels, pixmaps, and encoding.
- **GEO** — geometry and reprojection.
- **TST** — measurement and conformance against MapServer, the browser, and WASM.

**Status legend:** 🟢 accepted · 🟡 proposed · 🔴 rejected · ⚪ superseded

### Binding

The decisions in force: accepted, and proposed ones that bind the work implementing them. This
is the table a session reads.

| ADR | Title | Applies to | Status |
|-----|-------|------------|--------|
| [API-browser-binds-geojson-only](API-browser-binds-geojson-only.md) | The browser bindings bind only the GeoJSON a page passes, never a shapefile | `crates/bindings-wasm`: which feature sources it binds to the layers, and its dependencies on `adapter-<kind>` crates | 🟢 accepted |
| [API-geojson-through-georust](API-geojson-through-georust.md) | The GeoJSON adapter parses with the georust `geojson` crate | `crates/adapter-geojson` | 🟢 accepted |
| [API-init-projections-through-resolver](API-init-projections-through-resolver.md) | `init=` projections are resolved through a resolver port the host passes in, and from an init file the mapfile names by a path inside the base context | `crates/ports`, `crates/mapfile`, `crates/adapter-epsg`, and the hosts `crates/cli` and `crates/bindings-wasm` | 🟢 accepted |
| [API-shapefile-reader-from-mapserver](API-shapefile-reader-from-mapserver.md) | The shapefile adapter reads shapefiles with a reader derived from MapServer's | `crates/adapter-shapefile`, `docs/semantics/shapefile.md` | 🟢 accepted |
| [API-skipped-records-reported](API-skipped-records-reported.md) | A feature source reports the records it skips, and the engine turns them into one warning per layer | the `FeatureSource` port of `crates/ports`, its call in `crates/engine`, and every `adapter-<kind>` crate that implements it | 🟢 accepted |
| [API-source-declares-projection](API-source-declares-projection.md) | A feature source declares its projection through the port, and the shapefile adapter turns the WKT of its `.prj` into a proj4 string with a parser of its own | the `FeatureSource` port of `crates/ports` and its declared projection, its use by a layer of `PROJECTION AUTO` in `crates/engine`, the `.prj` reader of `crates/adapter-shapefile` and its fuzz target `fuzz/fuzz_targets/prj.rs`, and the declaration of `crates/adapter-geojson` | 🟢 accepted |
| [API-truetype-marker-through-port](API-truetype-marker-through-port.md) | A TrueType symbol at a point is drawn from a glyph symbolize asks of a port, which the engine answers through label | the TrueType marker of `crates/symbolize` and the glyph port it defines, the engine's answer to that port in `crates/engine` with its fonts, and the reading of a single glyph in `crates/label` | 🟢 accepted |
| [API-typed-feature-properties](API-typed-feature-properties.md) | A feature carries its properties as typed values, a shapefile's as its record in a `.dbf` its layer shares | the `Feature` type of `crates/ports`, `crates/adapter-geojson`, `crates/adapter-shapefile`, and every `adapter-<kind>` crate that delivers features | 🟢 accepted |
| [ARCH-crates-as-ports-and-adapters](ARCH-crates-as-ports-and-adapters.md) | The code is a Cargo workspace of one crate per responsibility, shaped as ports and adapters around a functional core, with the allowed dependency edges decided by a test | `Cargo.toml`, every crate under `crates/`, the `[dependencies]` of each, and the dependency test of `crates/arch-test` | 🟢 accepted |
| [ARCH-one-clip-to-image](ARCH-one-clip-to-image.md) | The clip to the image is one module of `render`, shared by the hatch's bound and every renderer | `crates/render/src/clip.rs`, the clipping of paths in `crates/render-agg`, and every renderer or bound that measures or clips a path against the image | 🟢 accepted |
| [ARCH-third-party-failures-guarded](ARCH-third-party-failures-guarded.md) | A third-party crate's failure on a user's input is kept out of a render by a guard Forager derives from the pinned version, and a guard that leaves a part undrawn warns | every guard in Forager's code that keeps an input from a third-party crate's computation where the crate panics, loops, overflows, or runs out of memory: `crates/render/src/clip.rs`, `crates/render-agg/src/brush.rs`, `crates/render-agg/src/glyphs.rs`, `crates/render-agg/src/lib.rs`, `crates/symbolize/src/geometry.rs`, `crates/symbolize/src/layer_transform.rs`, the diagnostics of a refused part in `crates/engine`, and `Cargo.lock` for the crates the guards name | 🟢 accepted |
| [GEO-map-rotation-in-reprojection](GEO-map-rotation-in-reprojection.md) | A map's `ANGLE` is MapServer's geotransform, a step of every layer's reprojection in `proj`, and a rotated map is drawn in the frame of its image | the geotransform of a rotated map in `crates/proj` and its use by the reprojection of a layer's features and of the area asked of its source (`crates/proj/src/rect.rs`, `crates/proj/src/shape.rs`, `crates/engine/src/reproject.rs`), the view, the scale, and the scale factors of a rotated map in `crates/engine` and the `View` of `crates/symbolize/src/extent.rs` | 🟢 accepted |
| [GEO-own-circle-for-buffer](GEO-own-circle-for-buffer.md) | `simplify` and `buffer` are computed in `symbolize`, by a Douglas-Peucker and a point's circle ported from JTS and with the geo crate, and a simplified polygon is repaired as GEOS repairs it | the geometry module of `crates/symbolize` that computes `simplify` and `buffer` and its callers in `crates/symbolize/src/layer_transform.rs`, `crates/symbolize/Cargo.toml`, `Cargo.lock` for geo and the crates it brings, `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of JTS, and the unit tests that hold coordinates taken from GEOS | 🟢 accepted |
| [GEO-proj-ported-with-libm](GEO-proj-ported-with-libm.md) | The core crate `proj` reprojects with its own port of PROJ's formulas, every transcendental function taken from `libm`, and no crate beyond it | `crates/proj`, its `[dependencies]`, the reprojection of a layer's features and of the area asked of its source in `crates/engine`, the workspace's `clippy.toml` and the three crate copies of it (`crates/engine/clippy.toml`, `crates/label/clippy.toml`, `crates/symbolize/clippy.toml`), `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of PROJ, the fuzz target of the reprojection `fuzz/fuzz_targets/proj.rs`, and the EPSG table the msautotest harness `crates/cli/tests/msautotest.rs` passes to the CLI | 🟢 accepted |
| [GEO-simplifypt-ported-from-jts](GEO-simplifypt-ported-from-jts.md) | `simplifypt` is computed in `symbolize` by a port of JTS's `TopologyPreservingSimplifier` | the code of `simplifypt` in the geometry module of `crates/symbolize` and its callers in `crates/symbolize/src/layer_transform.rs` and the drawing of a class style's tree, the notice of JTS on the files it ports, and the unit tests that hold coordinates taken from GEOS for it | 🟢 accepted |
| [IR-c-ports-in-style-ir](IR-c-ports-in-style-ir.md) | C's and MapServer's small ports, the `(int)` cast among them, live once in `style-ir` | the ports of C's `atof`, `strtod`, `atoi`, `lrint`, `(int)` cast of a `double`, and string end, and of MapServer's `MS_NINT`, the opacity alpha `MS_NINT(opacity * 2.55)` kept in a byte, `MS_MIN`, `MS_MAX`, `MS_DEG_TO_RAD`, `msRectOverlap`, `msComputeBounds`, `msStringSplit`, and `msGetSymbolIndex` with the key of its `strcasecmp`, in `crates/style-ir`, and their callers in `crates/expr`, `crates/mapfile`, `crates/engine`, `crates/symbolize`, `crates/label`, `crates/render-agg`, `crates/proj`, `crates/adapter-shapefile`, and `crates/adapter-geojson`; every conversion of a number that may truncate in `crates/engine`, `crates/symbolize`, and `crates/label` | 🟢 accepted |
| [IR-expressions-as-cql2](IR-expressions-as-cql2.md) | Expressions in the style IR are a lean CQL2 tree of Forager's own types, serialized as CQL2-JSON, with MapServer's typing made explicit at import and its patterns read as PCRE2 reads them | `crates/style-ir`, the translation of expressions in `crates/mapfile`, and every crate that evaluates an expression of the style IR | 🟢 accepted |
| [IR-expressions-evaluated-as-mapserver](IR-expressions-evaluated-as-mapserver.md) | The core crate `expr` evaluates the IR's expressions as MapServer's parser does, with patterns matched by the `regex` crate in byte mode | `crates/expr`, the selection of classes and the per-feature properties of `crates/engine`, and every crate that evaluates an expression of the style IR | 🟢 accepted |
| [IR-geomtransform-with-simplifypt](IR-geomtransform-with-simplifypt.md) | A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, `simplify`, `simplifypt`, and `buffer` among its nodes | the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.1.json` | 🟢 accepted |
| [IR-inline-features-in-layer](IR-inline-features-in-layer.md) | A layer's inline features are a field of the layer in the style IR, read by the engine as the layer's source | the inline features of a layer in `crates/style-ir`, the reading of `FEATURE` in `crates/mapfile`, and the source the engine makes of them in `crates/engine` | 🟢 accepted |
| [IR-schema-per-format-version](IR-schema-per-format-version.md) | The style document is versioned by a JSON Schema per format version, moved by the version rule until a release tag holds its schema and frozen from then, a newer minor read leniently by dropping what is unknown at its path, an older major migrated in steps, and a change of meaning a major | `crates/style-ir` (the style document, its readers in `document.rs`, `lenient.rs`, and `migration.rs`, its `schema` feature), `schema/` and `crates/style-ir/tests/schema.rs`, the `serde_ignored`, `serde_path_to_error`, and `schemars` entries of `Cargo.toml`, every read error of the style document that names a path, and every change to a field of the style IR | 🟢 accepted |
| [IR-serde-json-and-yaml](IR-serde-json-and-yaml.md) | The style document is serialized through serde, as JSON with `serde_json` and as YAML with `serde-saphyr` | `crates/style-ir` and every crate that reads or writes a style document | 🟢 accepted |
| [OPS-actions-by-major-tag](OPS-actions-by-major-tag.md) | Workflows reference actions by major version tag, as the template ships them | every `uses:` line under `.github/workflows/`, `.github/dependabot.yml`, the `sensors` job of `.github/workflows/checks.yml`, and the sensor of the action references `scripts/sensor-action-refs.sh` with its self-test `scripts/test-sensor-action-refs.sh` | 🟢 accepted |
| [OPS-backlog-commits-are-sensed](OPS-backlog-commits-are-sensed.md) | A sensor lists each commit that adds a backlog entry beside a change outside the backlog | `scripts/sensor-backlog-commits.sh` and its self-test | 🟢 accepted |
| [OPS-container-reaches-no-engine](OPS-container-reaches-no-engine.md) | The Dev Container gives no access to any container engine, not even one inside it | the `features`, `mounts`, and `runArgs` of `.devcontainer/devcontainer.json` | 🟢 accepted |
| [OPS-dev-container-adds-rust](OPS-dev-container-adds-rust.md) | The Dev Container adds Rust to the template's Debian base image through a locked Feature, and installs the pinned tools on creation | the `features` and the `postCreateCommand` of `.devcontainer/devcontainer.json`, and `.devcontainer/devcontainer-lock.json` | 🟢 accepted |
| [OPS-measurements-are-no-gate](OPS-measurements-are-no-gate.md) | A workflow that measures on a schedule is no gate, and a script only it runs stands on an `# On instruction:` line | `.github/workflows/fuzz.yml`, `.github/workflows/mutants.yml`, `.github/workflows/noise.yml`, the `# Not a gate:` and `# On instruction:` lines of `scripts/check-all.sh`, and the rule of those lines in `scripts/check-docs.sh` | 🟢 accepted |
| [OPS-parallel-sessions-capped-at-three](OPS-parallel-sessions-capped-at-three.md) | Tickets are worked in parallel by Claude Code sessions under a cap of three, claimed locally, each reviewed in a fresh session, with the threat-protected parts and public interfaces never worked unattended | every session that implements or reviews a change, `docs/plan/`, the `work-tickets` and `review` skills, and the worktrees under `.scratch/` | 🟢 accepted |
| [OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md) | The toolchain is pinned stable Rust with its cargo tools, the wasm-bindgen CLI, and the ShellCheck and jscpd release binaries, installed by `scripts/install-tools.sh` and run by the checks, with every sensor `scripts/check-all.sh` runs | `rust-toolchain.toml`, `.cargo/`, `rustfmt.toml`, `Cargo.toml`, `Cargo.lock`, `clippy.toml`, `crates/engine/clippy.toml`, `crates/label/clippy.toml`, `crates/symbolize/clippy.toml`, `deny.toml`, `scripts/install-tools.sh`, `scripts/install-shellcheck.sh`, `scripts/install-jscpd.sh`, the installer of pinned release binaries both use `scripts/lib/install.sh`, `scripts/check-rust.sh`, the ShellCheck of `scripts/check-shell.sh` and its self-test, the sensors `scripts/check-all.sh` runs (`scripts/sensor-tests-kept.sh`, `scripts/sensor-refactor-tests.sh`, `scripts/sensor-backlog-commits.sh`, `scripts/sensor-binding-adrs.sh`, `scripts/sensor-plain-rules.sh`, `scripts/sensor-revision-due.sh`, `scripts/sensor-plan-frontier.sh`, `scripts/sensor-backlog.sh`, `scripts/sensor-diagnostics-untested.sh`, `scripts/sensor-action-refs.sh`, `scripts/sensor-review-binding.sh`, `scripts/sensor-duplication.sh`) and their self-tests (`scripts/test-sensor-tests-kept.sh`, `scripts/test-sensor-refactor-tests.sh`, `scripts/test-sensor-backlog-commits.sh`, `scripts/test-sensor-binding-adrs.sh`, `scripts/test-sensor-plain-rules.sh`, `scripts/test-sensor-revision-due.sh`, `scripts/test-sensor-plan-frontier.sh`, `scripts/test-sensor-backlog.sh`, `scripts/test-sensor-diagnostics-untested.sh`, `scripts/test-sensor-action-refs.sh`, `scripts/test-sensor-review-binding.sh`, `scripts/test-sensor-duplication.sh`), the slow sensors `scripts/fuzz.sh` and `scripts/mutants.sh` with their self-tests `scripts/test-fuzz.sh` and `scripts/test-mutants.sh` and the fuzz targets `fuzz/`, `.jscpd.json`, the `postCreateCommand` of `.devcontainer/devcontainer.json`, the `rust`, `shell`, and `sensors` jobs of `.github/workflows/checks.yml`, the scheduled workflows of the slow sensors (`.github/workflows/fuzz.yml`, `.github/workflows/mutants.yml`), and the **Build, Test & Run** section of `README.md` | 🟢 accepted |
| [REN-agg-blender-and-resampler](REN-agg-blender-and-resampler.md) | `render-agg` blends and resamples with ports of the AGG 2.4 that MapServer bundles, while agg-rust rasterizes | the pixel format of `crates/render-agg` (how a covered pixel is blended, how a layer's image is laid over the map, how the alpha is divided out of the finished image) and the resampling of a pixmap it draws scaled or turned (its span generator, its interpolator, and the rounding of both) | 🟢 accepted |
| [REN-agg-rust-rasterizer](REN-agg-rust-rasterizer.md) | The reference renderer rasterizes with agg-rust in `render-agg`, behind clipping that keeps every coordinate near the image, with `unsafe` allowed in that crate alone | `crates/render`, `crates/render-agg`, and the `unsafe_code` lint of the workspace | 🟢 accepted |
| [REN-i-overlay-shared-with-geo](REN-i-overlay-shared-with-geo.md) | The hatch is cut from its polygon by i_overlay's boolean intersection, as MapServer's Clipper cuts it, and geo is the only other package that depends on i_overlay | the cut of a hatch in `crates/render-agg`, the `i_overlay` entry of its `Cargo.toml`, the i_overlay that geo brings into `crates/symbolize`, the bounds of a hatch in `crates/render`, and the test `only_render_agg_cuts_polygons_with_i_overlay` of `crates/arch-test` | 🟢 accepted |
| [REN-label-cache-wrapping-grid](REN-label-cache-wrapping-grid.md) | The label cache tests collisions over a grid that wraps around the image, with MapServer's outcome, and holds at most 100,000 labels a render | the label cache of `crates/label` and the engine's use of it in `crates/engine`: how a label is tested against the markers and the labels drawn before it, the grid that bounds those tests, and how many labels one render holds | 🟢 accepted |
| [REN-label-glyphs-from-skrifa](REN-label-glyphs-from-skrifa.md) | Label glyphs are drawn from skrifa's unhinted FreeType outlines, laid out unshaped as MapServer lays out its Latin lines | `crates/label`, the text of `crates/engine` and `crates/render-agg`, the glyph operation of `crates/render`, the resource loader each host hands the engine, and every crate that lays out, measures, or draws text | 🟢 accepted |
| [REN-pixmaps-held-once-per-render](REN-pixmaps-held-once-per-render.md) | A decoded pixmap is held once per render and per path, and shared by every item that draws it | the image store of `crates/engine`, the type `PremultipliedImage` of `crates/render` and the display items that carry it, the pixmap decoders of `crates/encode`, and every crate that draws a pixmap or a fill tile | 🟢 accepted |
| [REN-png-through-image-rs](REN-png-through-image-rs.md) | The encoder writes PNG with the image-rs `png` crate | `crates/encode` | 🟢 accepted |
| [REN-symbol-line-brushed-by-tile](REN-symbol-line-brushed-by-tile.md) | A symbol along a line without a GAP brushes the line with its tile, through agg-rust's outline-image rasterizer and a port of AGG 2.4's pattern filter | the drawing of a line style whose symbol is a vector, ellipse, pixmap, or TrueType symbol and whose `GAP` is 0 in `crates/symbolize`, the display item of a line drawn with a tile in `crates/render`, its rasterizing, its pattern filter, and its clip in `crates/render-agg` | 🟢 accepted |
| [SEC-crates-locked-and-denied](SEC-crates-locked-and-denied.md) | Crate dependencies come only from crates.io, are locked in `Cargo.lock`, and pass `cargo deny` for licences, advisories, and sources | `Cargo.lock`, `deny.toml`, and every `[dependencies]` entry of the workspace | 🟢 accepted |
| [SEC-gh-login-stays-on-host](SEC-gh-login-stays-on-host.md) | The GitHub CLI's login lives in a host directory of the container's own and outlives every rebuild | the `mounts`, `initializeCommand`, and `postCreateCommand` of `.devcontainer/devcontainer.json`, and `scripts/check-devcontainer.sh` | 🟢 accepted |
| [SEC-no-secrets-of-its-own](SEC-no-secrets-of-its-own.md) | The project holds no secrets of its own, and secret scanning with push protection is the only control | the repository, its workflows, and their `secrets.*` references | 🟢 accepted |
| [SEC-third-party-files-vendored](SEC-third-party-files-vendored.md) | The browser example loads every third-party file, its scripts, stylesheets, and fonts, only from copies vendored in the tree and checked by checksum | `examples/web/`, its `vendor/` directory, every third-party script, stylesheet, and font the example loads, and `scripts/check-vendor.sh` | 🟢 accepted |
| [TST-browser-test-over-webdriver](TST-browser-test-over-webdriver.md) | The browser test of G-14 drives headless Chrome over WebDriver with a client of its own, compares the canvas with the native render, and runs in a CI job of its own | the browser test `crates/dev-server/tests/browser.rs` and its WebDriver client, the dev-dependencies of `crates/dev-server` and their check in `crates/arch-test/tests/dependencies.rs`, the script `scripts/check-browser.sh` that runs the test, the job of the browser test in `.github/workflows/checks.yml` and the line of `scripts/check-all.sh` that run that script, and the variable `FORAGER_BROWSER_TEST` that job sets | 🟢 accepted |
| [TST-noise-measured-against-mapserver](TST-noise-measured-against-mapserver.md) | CI builds MapServer from the pinned tag to measure its noise, and reads its label cache with a C reader linked to that build | `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`, every run of MapServer itself, `crates/cli/tests/conformance/mapserver_labels.c`, the harness's noise module `crates/cli/tests/conformance/noise/`, `scripts/check-notice.sh`, the records of MapServer's labels under `crates/cli/tests/conformance/labels/` and the mark `labels-hold` of `crates/cli/tests/conformance/cases.txt`, and every comparison of label positions with MapServer's | 🟢 accepted |
| [TST-proj-references-from-mapserver](TST-proj-references-from-mapserver.md) | The reference points, rectangles, and shapes of G-11 are reprojected by MapServer's own calls of PROJ at the pinned build, in the noise run, and kept as records beside the test of `proj` | `crates/proj/tests/reference/` (the lists `points.tsv`, `rects.tsv`, and `shapes.tsv` and their records `mapserver.tsv`, `mapserver-rects.tsv`, and `mapserver-shapes.tsv`), the test `crates/proj/tests/reference.rs`, `crates/cli/tests/conformance/mapserver_proj.c`, its build in `scripts/build-mapserver.sh` and that script's self-test `scripts/test-build-mapserver.sh`, its run in `.github/workflows/noise.yml` and the harness's noise module `crates/cli/tests/conformance/noise/`, and the paths that start that workflow | 🟢 accepted |
| [TST-wasm-renders-in-wasmi](TST-wasm-renders-in-wasmi.md) | The conformance test runs the engine's wasm32 build in wasmi and compares it with the native render | the test of G-9 in `crates/cli/tests/`, its `wasmi` dev-dependency and wasmi's optimization in the dev profile, the exports of `crates/bindings-wasm` behind its feature `conformance`, the build of that module in `scripts/check-rust.sh`, and `cargo deny` for the crates it adds | 🟢 accepted |

### Superseded and rejected

The record of what was decided against or replaced: read one when a change touches what it
governed, to learn why the current decision stands. A row moves here in the change that
flips its status.

| ADR | Title | Applies to | Status |
|-----|-------|------------|--------|
| [GEO-simplify-buffer-through-geo](GEO-simplify-buffer-through-geo.md) | `simplify` and `buffer` are computed in `symbolize`, by a Douglas-Peucker ported from JTS and with the geo crate, and a simplified polygon is repaired as GEOS repairs it | the geometry module of `crates/symbolize` that computes `simplify` and `buffer` and its callers in `crates/symbolize/src/layer_transform.rs`, `crates/symbolize/Cargo.toml`, `Cargo.lock` for geo and the crates it brings, `scripts/check-notice.sh` and `THIRD-PARTY-NOTICES.md` for the notice of JTS, and the unit tests that hold coordinates taken from GEOS | ⚪ superseded by GEO-own-circle-for-buffer |
| [IR-geomtransform-with-simplify-buffer](IR-geomtransform-with-simplify-buffer.md) | A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, `simplify` and `buffer` among its nodes | the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.1.json` | ⚪ superseded by IR-geomtransform-with-simplifypt |
| [IR-typed-geomtransform](IR-typed-geomtransform.md) | A `GEOMTRANSFORM` expression is a typed tree of its shape functions in the style IR, translated by the mapfile frontend | the transform tree of `crates/style-ir` (a style's `GeomTransform::Expression` and a layer's `geom_transform`), the translation of `GEOMTRANSFORM` expressions in `crates/mapfile`, every crate that evaluates or reports a transform tree (`crates/engine`, `crates/symbolize`), and `schema/style-document-1.1.json` | ⚪ superseded by IR-geomtransform-with-simplify-buffer |
| [REN-hatch-cut-by-i-overlay](REN-hatch-cut-by-i-overlay.md) | The hatch is cut from its polygon by i_overlay's boolean intersection, as MapServer's Clipper cuts it | the cut of a hatch in `crates/render-agg`, the `i_overlay` entry of its `Cargo.toml`, and the bounds of a hatch in `crates/render` | ⚪ superseded by REN-i-overlay-shared-with-geo |
