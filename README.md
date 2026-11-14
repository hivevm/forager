# Forager — a MapServer-compatible render core in Rust

[![Checks](https://github.com/hivevm/forager/actions/workflows/checks.yml/badge.svg)](https://github.com/hivevm/forager/actions/workflows/checks.yml)

**Forager** renders maps with the styling semantics of UMN MapServer: it reads mapfiles and
rasterizes them itself, in the browser as WebAssembly and on the server as a Rust library. It is
built **with coding agents** inside a ready-to-use Dev Container.
The work is driven by a written **specification**
([`docs/SPECIFICATION.md`](docs/SPECIFICATION.md)) and **Architecture Decision Records**
([`docs/adr/`](docs/adr/)), so intent and the reasoning behind every structural choice stay
explicit and reviewable.

> For agent instructions, see [`AGENTS.md`](AGENTS.md), the single source of truth for all coding
> agents.

## Overview

Forager renders maps with the styling semantics of UMN MapServer. Its mapfile frontend reads a
mapfile into a style IR free of MapServer syntax; a core without I/O or C code renders that IR,
as WebAssembly in the browser and as a Rust library on a server. It is for teams that maintain
their cartography as mapfiles and want it in the browser without a map server. The full problem
statement and goals live in [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md), the vocabulary in
[`docs/GLOSSARY.md`](docs/GLOSSARY.md).

## Prerequisites

- [VS Code](https://code.visualstudio.com/) with the
  [Dev Containers](https://marketplace.visualstudio.com/items?itemName=ms-vscode-remote.remote-containers)
  extension, or any DevContainer-compatible IDE
- Docker / Podman (rootless) available on the host
- Outside the Dev Container: bash 4.4 or later for the checks, 3.2 for the git hooks
  ([ADR-0002](docs/adr/0002-decisions-verified-by-tests.md)); macOS ships 3.2

## Getting Started

1. Open the repository in VS Code and choose **Reopen in Container**. The Dev Container and the
   preconfigured agent extensions build automatically, and the container enables the repository's
   git hooks with the merge settings they rely on ([Git hooks](CONTRIBUTING.md#git-hooks)). Working
   outside the container? Run the commands listed there yourself, once per clone.
2. Authenticate your coding agent inside the container (for Claude Code: `claude login`).
3. Start working with the agent. Drive the work from the specification and the ADRs.

## Build, Test & Run

The toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); rustup installs it on
first use. [`scripts/install-tools.sh`](scripts/install-tools.sh) installs it with the cargo
tools the checks need and the release binaries of ShellCheck and jscpd, and the Dev Container runs
it after creation (ADR-0007, ADR-0009, ADR-draft-sensors-and-duplication).

- **Build:** `cargo build --workspace`
- **Test:** `cargo test --workspace`
- **Lint:** `bash scripts/check-rust.sh` — formatting, clippy, tests, the `wasm32-unknown-unknown`
  build, the WebAssembly bundle and its size, and `cargo deny`; `scripts/check-all.sh` runs it with
  the repository's other checks
- **Duplication:** `bash scripts/sensor-duplication.sh` — jscpd's report of the clones in the
  Rust and bash files, each with its two places; it fails on no count, and `scripts/check-all.sh`
  runs it
- **Run:** `cargo run -p forager-cli -- read <mapfile>`, or
  `cargo run -p forager-cli -- render <mapfile> --source <layer>=<file.geojson> -o <image.png>`
- **Browser example:** `bash scripts/build-web.sh`, then `cargo run -p forager-dev-server` and open
  <http://127.0.0.1:8000/>; in VS Code, the launch configuration *Browser example* does all three
- **Package:** `bash scripts/package.sh` — the CLI's archive and the browser bindings' package
  under `target/package/`, each carrying `THIRD-PARTY-NOTICES.md`, `LICENSE`, and
  `LICENSE-APACHE`, and checked for them; nothing is published
- **Fuzz:** `bash scripts/fuzz.sh [seconds per target]` — the weekly fuzz run of Q-3, locally; it
  installs its nightly toolchain and cargo-fuzz
- **Mutants:** `bash scripts/mutants.sh [shard such as 1/20]` — the weekly mutation testing of
  ADR-0007, locally; it reports the mutants no test caught

## Usage

Read a mapfile and print the style IR it translates into, with every diagnostic on standard
error:

```console
$ forager read crates/cli/tests/fixtures/world.map --format yaml
world.map:12:5: warning [unsupported-keyword]: METADATA is not yet supported and is ignored
layers/countries.inc:6:3: warning [unsupported-keyword]: GROUP is not yet supported and is ignored
version: "1.0"
map:
  name: world
  …
```

`--format json` prints the same document as JSON. The exit code is 0 when the mapfile loaded, 1
when it did not, and 2 on wrong arguments. `INCLUDE`s are resolved inside the mapfile's
directory, and a path that leaves it is refused.

`init=epsg:` projections resolve without further input for EPSG:4326, EPSG:3857, and the WGS84
UTM zones; for other codes pass a table in the format of PROJ's former `epsg` init file, lines
`<27700> +proj=tmerc … <>`, with `--epsg <file>` (the browser's `render` takes its text as a last
argument). Forager ships no EPSG data beyond those built-in codes.

Render a mapfile's layers from GeoJSON files, bound to the layers by name, into a PNG:

```console
$ forager render crates/cli/tests/fixtures/square.map \
    --source areas=crates/cli/tests/fixtures/areas.geojson -o square.png
square.map:23:5: warning [layer-skipped]: layer `unbound` (source local) is skipped: no source is bound to it
```

The browser example below shows more of what the engine draws: classes picked by expression,
scale ranges, symbols along lines and at their ends, dashes, hatches, opacity, and labels that
the label cache places by priority.

Render in the browser: build the WebAssembly bundle, start the local server, and open
<http://127.0.0.1:8000/>. The page loads a sample mapfile with GeoJSON for its five layers, a
synthetic landscape of land use, rivers, buildings, roads, and a few hundred named places that
`scripts/generate-example.sh` generates from a fixed seed, and the font set and font its labels
use; edit the mapfile or the GeoJSON and press **Render**. At the top of the page, a
[Leaflet](https://leafletjs.com/) map pans and zooms over the same map, from zoom 0, one map unit
a pixel, with streets and houses a few levels in, each tile rendered by Forager for its extent;
Leaflet and the font DejaVu Sans are checked copies under `examples/web/vendor/`
([ADR-0035](docs/adr/0035-vendored-third-party-files.md)).

```console
$ bash scripts/build-web.sh
$ cargo run -p forager-dev-server
Serving the Forager example on http://127.0.0.1:8000/
```

From JavaScript, the bundle's `render(mapfile, sources, width?, height?, extent?)` takes the
mapfile text and a JSON object from layer name to GeoJSON (`renderWith` takes as well the
`Files` beside the mapfile, such as its font set and fonts), and returns the image's `width`,
`height`, RGBA `pixels`, and its `diagnostics` as JSON. A source reference that carries credentials
is refused in the browser.

## Template

- **Template release:** v0.8.1 — the release of [NUC](https://github.com/hivevm/nuc) this
  repository carries; a project moves the line when it takes up a later one
  ([ADR-0004](docs/adr/0004-template-releases.md)).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the workflow (specification- and ADR-driven, small
reviewable changes), the project layout, the Dev Container, the coding agents, the git hooks, the
integration branches, and the repository settings, and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)
for the community standards we expect of everyone taking part. Security issues: please follow
[`SECURITY.md`](SECURITY.md) instead of opening a public issue.

## License

Released under `MIT OR Apache-2.0`. See [`LICENSE`](LICENSE) (MIT) and
[`LICENSE-APACHE`](LICENSE-APACHE). The notices of the code Forager follows, MapServer's and
W. Randolph Franklin's pnpoly, are in [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md), which
every distribution of Forager must carry, one of its binaries included.
