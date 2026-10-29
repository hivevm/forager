# Architecture Decision Records

This directory contains all Architecture Decision Records (ADRs) for this project. Accepted ADRs are
**binding** for humans and coding agents alike (see [`AGENTS.md`](../../AGENTS.md) in the repository
root). ADRs derive from the specification in [`docs/SPECIFICATION.md`](../SPECIFICATION.md).

## Mechanics

The rules live in [`AGENTS.md` §3](../../AGENTS.md#3-adr-rules): when an ADR is written, how it
is developed, who flips its status, that an accepted one is immutable and changed only by
supersession. This section carries only the mechanics of the record;
[`scripts/check-docs.sh`](../../scripts/check-docs.sh) names in its header which of them it
verifies.

- **File.** Copy [`template.md`](template.md) to `NNNN-short-title.md` with the next free number.
  The `# ADR-NNNN` heading matches the filename, and the numbers run `0001..N` without gaps.
- **Index.** The tables below change in the same pull request as the ADR it describes, for
  additions, supersessions, and status flips alike. The status is shown via the legend's bullet
  and the ADR's `Applies to` header is mirrored verbatim in its own column. The index routes a
  reader from a change to the decisions that bind it, so a row says what its ADR governs without
  the file being opened. Two tables keep what a session reads apart from what it looks up: an
  accepted or proposed ADR sits under **Binding**, a superseded or rejected one under
  **Superseded and rejected**, and a status flip moves the row.
- **Numbers are permanent once on `main`, until a normalization.** Between two normalizations,
  never renumber, delete, or merge an ADR there: other ADRs, commits (`Implements ADR-NNNN`), and
  code may reference a number. A proposed ADR whose number another pull request brought to
  `main` first takes the next free number before it merges, in its file name, heading, and index
  row and in every reference its own branch makes to it. Until a normalization, a superseded ADR
  keeps its file and its body. Its `Status` line flips to `⚪ superseded by ADR-NNNN` in the pull
  request that lands the superseding ADR, whose `Supersedes` field names it back.
- **Between normalizations, sprawl is curbed by supersession, never by editing.** One ADR may
  supersede several whose decisions have grown into one. Its `Supersedes` field names each, and
  each flips its status in the same pull request, so the set an agent reads for a change shrinks
  while every number and every body stays.
- **A normalization restates the record** ([`AGENTS.md` §3](../../AGENTS.md#3-adr-rules), rule
  5).
  - **Start.** The tree has no proposed ADR, no feature spec or ticket under `docs/plan/`, and
    green checks.
  - **Steps.** Three pull requests, each landed by a human: the restated specification, whose
    `G-n` and `Q-n` run from 1; the supersessions it calls for; and the restated record.
  - **Order.** The record numbers the ADRs in force `0001..N` by what they govern. The document
    set and the ADR process come first, then the founding decisions in the order of the setup,
    then the rest in groups. An inherited ADR takes its place like any other, and may be merged
    with the project's own.
  - **Clauses.** Every clause of an old ADR lives on in one new ADR. The pull request of the
    record lists each clause with the new ADR that carries it.
  - **Header.** A new ADR keeps the status of the ADRs it restates. Its `Deciders` name all of
    theirs, its `Date` is the latest of theirs, and it has no `Supersedes` field. In an
    adoption, an ADR the project had accepted stays accepted. A decision found outside any
    record enters as proposed.
  - **Dead ADRs.** Superseded and rejected ADRs are deleted. What one of them chose, and why it
    did not stand, becomes an entry under *Alternatives considered* in the new ADR that governs
    its topic.
  - **References.** Each step moves its references with a map, old to new, applied by the map
    script of the `normalize` skill. The map stands in the pull request description and the
    commit body, and in no document. A commit older than the normalization cites the numbers it
    had then.
- **Cite only what exists.** Every `ADR-NNNN` reference names a file already in this directory.
  An anticipated follow-up is described by topic ("a follow-up ADR on session storage"), never by
  a number. In Markdown, cite an ADR as a link to its own file.

## Index

An ADR whose `Deciders` line names the **NUC maintainer** is inherited from the template. It
binds a derived project once its own maintainer adds their name to `Deciders` and its `Status`
line reads `🟢 accepted` ([`AGENTS.md` §3](../../AGENTS.md#3-adr-rules)). Change an inherited
decision by superseding it, never by editing. Once the template setup is done, check 12 of
[`scripts/check-docs.sh`](../../scripts/check-docs.sh) fails on an inherited ADR in force that
names no decider of the project, or that is still proposed.

**Status legend:** 🟢 accepted · 🟡 proposed · 🔴 rejected · ⚪ superseded

### Binding

The decisions in force: accepted, and proposed ones that bind the work implementing them. This
is the table a session reads.

| ADR | Title | Applies to | Status |
|-----|-------|------------|--------|
| [0001](0001-agent-governance-model.md) | The document set: one specification, one ADR record, one `AGENTS.md`, one overview, one glossary, one conventions file | every document that carries rules for humans or agents, `AGENTS.md` among them, the architecture overview, the glossary, the conventions, `scripts/check-docs.sh`, `scripts/sensor-binding-adrs.sh`, and the map script of the `normalize` skill, `.agents/skills/normalize/remap.sh` | 🟢 accepted |
| [0002](0002-decisions-verified-by-tests.md) | Every accepted decision and every success criterion is verified by a test that cites it, and the checks that hold the decisions are bash scripts under `scripts/` | every accepted ADR, the Goals and Quality Goals of `docs/SPECIFICATION.md`, the tests that verify them, and the checks and sensors under `scripts/` | 🟢 accepted |
| [0003](0003-procedures-as-skills.md) | The procedures of the rule file are Agent Skills under `.agents/skills/`, each carrying a how and no rule of its own | `.agents/skills/`, the pointers under `.claude/skills/`, and every document that describes how a procedure of `AGENTS.md` is carried out | 🟢 accepted |
| [0004](0004-dev-container-runtime.md) | The Dev Container keeps the host daemon out of reach and its Features locked | `.devcontainer/`, `.vscode/settings.json`, `scripts/check-devcontainer.sh`, and everything the container pulls in | 🟢 accepted |
| [0005](0005-template-releases.md) | The template is released as SemVer tags on `main`, and every repository names the release it carries | the tags of the template repository, its own ADRs, the **Template** section of `README.md`, check 14 of `scripts/check-docs.sh`, and the pull request that lands a release | 🟢 accepted |
| [0006](0006-feature-layer.md) | Work larger than one session is planned in the tree as a feature spec cut into tracer-bullet tickets, files deleted when their work lands, and the design is revised when a spec closes or after a number of changes | every change larger than one agent session, `docs/plan/`, the spec and ticket templates of the `plan-feature` skill, the pull request template, the `Last design revision` line of `docs/ARCHITECTURE.md`, `scripts/sensor-revision-due.sh`, `scripts/sensor-plan-frontier.sh`, and the claim script `.agents/skills/work-tickets/claim.sh` | 🟢 accepted |
| [0007](0007-harness.md) | Tickets are worked in parallel by Claude Code sessions under a cap of three, claimed locally, each reviewed in a fresh session, with the threat-protected parts and public interfaces never worked unattended | every session that implements or reviews a change, `docs/plan/`, the `work-tickets` and `review` skills, and the worktrees under `.scratch/` | 🟢 accepted |
| [0008](0008-rust-toolchain.md) | The toolchain is pinned stable Rust with cargo, clippy, rustfmt, cargo-deny, and the wasm-bindgen CLI, installed by `scripts/install-tools.sh` and run by `scripts/check-rust.sh` | `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `scripts/install-tools.sh`, `scripts/check-rust.sh`, the `rust` job of `.github/workflows/checks.yml`, the scheduled workflow of the slow sensors, and the **Build, Test & Run** section of `README.md` | 🟢 accepted |
| [0009](0009-ports-and-adapters-crates.md) | The code is a Cargo workspace of one crate per responsibility, shaped as ports and adapters around a functional core, with the allowed dependency edges decided by a test | `Cargo.toml`, every crate under `crates/`, the `[dependencies]` of each, and the dependency test of `crates/arch-test` | 🟢 accepted |
| [0010](0010-rust-dev-container.md) | The Dev Container adds the locked Rust Feature to the Debian base image, runs `scripts/install-tools.sh` after creation, and gives no access to containers | `.devcontainer/devcontainer.json` and `.devcontainer/devcontainer-lock.json` | 🟢 accepted |
| [0011](0011-action-references-major-tag.md) | Workflows reference actions by major version tag, as the template ships them | every `uses:` line under `.github/workflows/`, `.github/dependabot.yml`, and the `sensors` job of `checks.yml` | 🟢 accepted |
| [0012](0012-crate-supply-chain.md) | Crate dependencies come only from crates.io, are locked in `Cargo.lock`, and pass `cargo deny` for licences, advisories, and sources | `Cargo.lock`, `deny.toml`, and every `[dependencies]` entry of the workspace | 🟢 accepted |
| [0013](0013-no-project-secrets.md) | The project holds no secrets of its own, and secret scanning with push protection is the only control | the repository, its workflows, and their `secrets.*` references | 🟢 accepted |
| [0014](0014-style-document-serde.md) | The style document is serialized through serde, as JSON with `serde_json` and as YAML with `serde-saphyr` | `crates/style-ir` and every crate that reads or writes a style document | 🟢 accepted |
| [0015](0015-agg-rust-rasterizer.md) | The reference renderer rasterizes with agg-rust in `render-agg`, behind clipping that keeps every coordinate near the image, with `unsafe` allowed in that crate alone | `crates/render`, `crates/render-agg`, and the `unsafe_code` lint of the workspace | 🟢 accepted |
| [0016](0016-geojson-crate.md) | The GeoJSON adapter parses with the georust `geojson` crate | `crates/adapter-geojson` | 🟢 accepted |
| [0017](0017-png-crate.md) | The encoder writes PNG with the image-rs `png` crate | `crates/encode` | 🟢 accepted |
| [0020](0020-style-document-compatibility.md) | The style document is versioned by a JSON Schema per format version, read leniently across minors, and migrated across majors in steps | `crates/style-ir` (the style document, its readers, its `schema` feature), the schema files of the style document and the check that compares them, and every change to the shape of the style IR | 🟢 accepted |
| [0021](0021-vendored-leaflet.md) | The browser example loads third-party JavaScript only from copies vendored in the tree and checked by checksum | `examples/web/`, its `vendor/` directory, and every JavaScript library the example loads | 🟢 accepted |
| [0022](0022-lenient-reader-path-to-error.md) | A newer minor's document that does not read is located with `serde_path_to_error` and read again with the failed value dropped, at most a hundred times | `crates/style-ir` (the lenient reader of the style document in `document.rs`), the `serde_path_to_error` entry of `Cargo.toml`, and every read error of the style document that names a path | 🟢 accepted |
| [0023](0023-projection-resolver-and-init-files.md) | `init=` projections are resolved through a resolver port the host passes in, and from an init file the mapfile names by a path inside the base context | `crates/ports`, `crates/mapfile`, `crates/adapter-epsg`, and the hosts `crates/cli` and `crates/bindings-wasm` | 🟢 accepted |
| [0024](0024-shapefile-reader.md) | The shapefile adapter reads shapefiles with a reader derived from MapServer's | `crates/adapter-shapefile`, `docs/semantics/shapefile.md` | 🟢 accepted |
| [0025](0025-source-reports-skipped-records.md) | A feature source reports the records it skips, and the engine turns them into one warning per layer | the `FeatureSource` port of `crates/ports`, its call in `crates/engine`, and every `adapter-<kind>` crate that implements it | 🟢 accepted |
| [0026](0026-feature-properties-at-the-port.md) | A feature carries its properties as typed values, a shapefile's as its record in a `.dbf` its layer shares | the `Feature` type of `crates/ports`, `crates/adapter-geojson`, `crates/adapter-shapefile`, and every `adapter-<kind>` crate that delivers features | 🟢 accepted |
| [0027](0027-mapserver-built-from-the-pin.md) | CI builds MapServer from the pinned tag to measure its noise | `scripts/build-mapserver.sh`, `.github/workflows/noise.yml`, and every run of MapServer itself | 🟢 accepted |
| [0028](0028-text-stack.md) | Label glyphs are drawn from skrifa's unhinted FreeType outlines, laid out unshaped as MapServer lays out its Latin lines | `crates/label`, the text of `crates/engine` and `crates/render-agg`, the glyph operation of `crates/render`, the resource loader each host hands the engine, and every crate that lays out, measures, or draws text | 🟡 proposed |
| [0029](0029-label-positions-from-mapservers-label-cache.md) | The noise measurement reads MapServer's label cache with a C reader linked to its library | `crates/cli/tests/conformance/mapserver_labels.c`, `scripts/build-mapserver.sh`, `scripts/check-notice.sh`, and every comparison of label positions with MapServer's | 🟡 proposed |
| [0030](0030-cql2-expressions.md) | Expressions in the style IR are a lean CQL2 tree of Forager's own types, serialized as CQL2-JSON, with MapServer's typing made explicit at import and its patterns read as PCRE2 reads them | `crates/style-ir`, the translation of expressions in `crates/mapfile`, and every crate that evaluates an expression of the style IR | 🟡 proposed |
| [0031](0031-expression-evaluator.md) | The core crate `expr` evaluates the IR's expressions as MapServer's parser does, with patterns matched by the `regex` crate in byte mode | `crates/expr`, the selection of classes and the per-feature properties of `crates/engine`, and every crate that evaluates an expression of the style IR | 🟡 proposed |

### Superseded and rejected

The record of what was decided against or replaced: read one when a change touches what it
governed, to learn why the current decision stands. A row moves here in the pull request that
flips its status.

| ADR | Title | Applies to | Status |
|-----|-------|------------|--------|
| [0018](0018-projection-resolver.md) | `init=` projections are resolved through a resolver port the host passes in, with a minimal built-in table and a host-supplied file for the rest | `crates/ports`, `crates/mapfile`, `crates/adapter-epsg`, and the hosts `crates/cli` and `crates/bindings-wasm` | ⚪ superseded |
| [0019](0019-cql2-expressions.md) | Expressions in the style IR are a lean CQL2 tree of Forager's own types, serialized as CQL2-JSON, with MapServer's typing made explicit at import | `crates/style-ir`, the translation of expressions in `crates/mapfile`, and every crate that evaluates an expression of the style IR | ⚪ superseded |
