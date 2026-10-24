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

## Getting Started


1. Open the repository in VS Code and choose **Reopen in Container**. The Dev Container and the
   preconfigured agent extensions build automatically, and the container enables the repository's
   git hooks ([`.githooks/`](.githooks/)): no commit on `main`, and the checks run before a push.
   Working outside the container? Enable them yourself, once per clone:
   `git config core.hooksPath .githooks`.
2. Authenticate your coding agent inside the container (for Claude Code: `claude login`).
3. Start working with the agent. Drive the work from the specification and the ADRs.

## Build, Test & Run

The toolchain is pinned in [`rust-toolchain.toml`](rust-toolchain.toml); rustup installs it on
first use. [`scripts/install-tools.sh`](scripts/install-tools.sh) installs it with the cargo
tools the checks need, and the Dev Container runs it after creation (ADR-0008, ADR-0010).

- **Build:** `cargo build --workspace`
- **Test:** `cargo test --workspace`
- **Lint:** `bash scripts/check-rust.sh` — formatting, clippy, tests, the `wasm32-unknown-unknown`
  build, the WebAssembly bundle and its size, and `cargo deny`; `scripts/check-all.sh` runs it with
  the repository's other checks
- **Run:** `cargo run -p forager-cli -- read <mapfile>`, or
  `cargo run -p forager-cli -- render <mapfile> --source <layer>=<file.geojson> -o <image.png>`
- **Browser example:** `bash scripts/build-web.sh`, then `cargo run -p forager-dev-server` and open
  <http://127.0.0.1:8000/>
- **Fuzz:** `bash scripts/fuzz.sh [seconds per target]` — the weekly fuzz run of Q-3, locally; it
  installs its nightly toolchain and cargo-fuzz
- **Mutants:** `bash scripts/mutants.sh [shard such as 1/20]` — the weekly mutation testing of
  ADR-0008, locally; it reports the mutants no test caught

## Usage

Read a mapfile and print the style IR it translates into, with every diagnostic on standard
error:

```console
$ forager read crates/cli/tests/fixtures/world.map --format yaml
world.map:11:3: warning [discarded-keyword]: WEB is out of Forager's scope and is discarded
layers/countries.inc:6:3: warning [unsupported-keyword]: LABELITEM is not yet supported and is ignored
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

The first class of each layer draws every feature for now; class expressions, symbols, labels,
and projections come with later work.

Render in the browser: build the WebAssembly bundle, start the local server, and open
<http://127.0.0.1:8000/>. The page loads a sample mapfile with GeoJSON for its three layers; edit
either and press **Render**.

```console
$ bash scripts/build-web.sh
$ cargo run -p forager-dev-server
Serving the Forager example on http://127.0.0.1:8000/
```

From JavaScript, the bundle's `render(mapfile, sources, width?, height?, extent?)` takes the
mapfile text and a JSON object from layer name to GeoJSON, and returns the image's `width`,
`height`, RGBA `pixels`, and its `diagnostics` as JSON. A source reference that carries credentials
is refused in the browser.

## Project Layout

```
README.md             # overview & setup for humans
AGENTS.md             # single source of truth for coding agents
docs/SPECIFICATION.md # the specification: problem, goals, success criteria
docs/GLOSSARY.md      # the vocabulary everyone uses, kept current inline
docs/CONVENTIONS.md   # how this project writes what no check decides, kept current inline
docs/ARCHITECTURE.md  # the system as it currently stands
docs/adr/             # Architecture Decision Records (+ template)
docs/plan/            # feature specs and their tickets, deleted as their work lands (ADR-0006)
examples/web/         # the browser example: a page, a sample mapfile, and its GeoJSON
fuzz/                 # the fuzz targets of Q-3, run weekly by scripts/fuzz.sh (own workspace)
Cargo.toml            # the Rust workspace: one crate per responsibility under crates/ (ADR-0009)
crates/               # the crates, one per responsibility: style-ir, mapfile, engine, cli, …
rust-toolchain.toml   # the pinned Rust toolchain (ADR-0008)
deny.toml             # cargo-deny: licences, advisories, sources (ADR-0012)
scripts/              # consistency checks and sensors (check-all.sh runs them all), run in CI
.githooks/            # git hooks: refuse a commit on main and a push while the checks are red
.github/              # CI workflows, Dependabot, issue & pull request templates, code owners
.devcontainer/        # Dev Container definition (base image + Features)
.vscode/              # shared editor settings
.editorconfig         # editor-neutral formatting baseline
.gitattributes        # line-ending normalization (LF everywhere)
.agents/skills/       # the procedures of AGENTS.md as Agent Skills, one directory each (ADR-0003)
.claude/CLAUDE.md     # pointer for Claude Code to read AGENTS.md
.claude/skills/       # pointers (one symlink per skill) for Claude Code to read .agents/skills/
.claude/settings.json # Claude Code permissions: deny reading .env files (see SECURITY.md)
```

## Dev Container

The environment is defined entirely in
[`.devcontainer/devcontainer.json`](.devcontainer/devcontainer.json). It starts from a prebuilt
base image and layers Dev Container Features and VS Code extensions on top; no Dockerfile or
Compose file is required. Customise the environment by adding Features, switching the base image,
or adding extensions. Features are pinned by major tag and resolved in the committed
`devcontainer-lock.json`. Extensions are listed by identifier only, because a published extension
version cannot be repointed and a version suffix is VS Code-specific.

### Host container management

The Dev Container deliberately has **no access to the host Docker daemon**: the socket is not
mounted ([ADR-0004](docs/adr/0004-dev-container-runtime.md)). To manage the host's containers from
VS Code, run the **Container Tools** extension (`ms-azuretools.vscode-containers`) on the **host**
side: install it in your host VS Code. [`.vscode/settings.json`](.vscode/settings.json) already pins
it to run locally via `remote.extensionKind`, so it keeps talking to the host engine even when this
folder is reopened in the container.

## Coding Agents

This Dev Container preinstalls the **Claude Code** and **Mistral Vibe** VS Code extensions (see
[`.devcontainer/devcontainer.json`](.devcontainer/devcontainer.json)); other agents (OpenAI Codex,
Cursor, OpenCode, GitHub Copilot) work too once you add them.

The rules every agent follows live in [`AGENTS.md`](AGENTS.md); that there is exactly one such file
is decided in [ADR-0001](docs/adr/0001-agent-governance-model.md). An agent that cannot read it
natively gets a pointer file instead. [`.claude/CLAUDE.md`](.claude/CLAUDE.md) is the one shipped
here; it carries the pointer and no rules of its own ([`AGENTS.md` §3](AGENTS.md#3-adr-rules)). It
exists for one gap only: Claude Code loads `CLAUDE.md`, not `AGENTS.md`, tracked upstream as
[anthropics/claude-code#34235](https://github.com/anthropics/claude-code/issues/34235). Once that
lands, delete the pointer file rather than keeping a second file to maintain.

The procedures those rules name (proposing an ADR, planning a feature, working its tickets
alone or in parallel, reviewing a change, revising the design, normalizing the record) are Agent
Skills under [`.agents/skills/`](.agents/skills/), the open format and the directory Codex,
Gemini CLI, and Cursor read
([ADR-0003](docs/adr/0003-procedures-as-skills.md)). An agent invokes one by name (`/review`) or
picks it up when the task matches its description. Claude Code reads `.claude/skills/` only, so
that directory holds one symlink per skill into `.agents/skills/`; a directory-level symlink is
not followed. The symlinks are pointers of the same kind as `CLAUDE.md`, tracked upstream as
[anthropics/claude-code#31005](https://github.com/anthropics/claude-code/issues/31005). Delete them
when that lands, and add one for every skill added meanwhile; the documentation check fails on a
skill without its pointer. A rule is enforced only by what every agent hits: the git hooks under
[`.githooks/`](.githooks/), the checks, and the repository settings below. A tool's own
configuration is not used to enforce one, because a gate that one agent honours and the next does
not looks like a rule and is not one.

Agents work tickets in parallel without any one vendor's mechanism: a dispatching session claims
each ticket by its branch in a git worktree of its own and starts a fresh session there, with the
non-interactive command of whichever agent the project's harness ADR names, and the session works
the ticket through the `work-tickets` skill.

## Repository settings

Some of this project's rules cannot be enforced by files in the repository
([`AGENTS.md` §6](AGENTS.md#6-project-rules)). They are settings on the GitHub repository itself,
configured once by a maintainer and worth re-checking after a repository move or transfer:

- a **ruleset on `main`** that requires pull requests, requires the
  <!-- required-checks begin — compared with the workflow's jobs by scripts/check-docs.sh -->
  `docs`, `traceability`, `devcontainer`, `sensors`, `shell`, and `rust`
  <!-- required-checks end -->
  jobs of the **Checks** workflow as required status checks (rulesets list checks by their job
  name) and a branch up to date with `main` before it merges, so that the checks see what the
  merge produces, such as two ADRs proposed in parallel under one number, and blocks force
  pushes and branch deletion;
- enable **require a review from Code Owners** on that ruleset, so the two artifacts only a human
  decides, [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md) and everything under
  [`docs/adr/`](docs/adr/) ([`AGENTS.md` §3](AGENTS.md#3-adr-rules)), cannot be merged without the
  owner named in [`.github/CODEOWNERS`](.github/CODEOWNERS), which has to carry a real handle or
  team for the requirement to mean anything;
- enable **secret scanning with push protection**, the only mechanical backstop behind the secrets
  rule ([`AGENTS.md` §7](AGENTS.md#7-secrets)), which is otherwise carried by review alone;
- enable **private vulnerability reporting** (see [`SECURITY.md`](SECURITY.md)).

## Template

- **Template release:** v0.2.0 — the release of [NUC](https://github.com/hivevm/nuc) this
  repository carries; a project moves the line when it takes up a later one
  ([ADR-0005](docs/adr/0005-template-releases.md)).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the workflow (specification- and ADR-driven, small
reviewable changes) and [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) for the community standards we
expect of everyone taking part. Security issues: please follow [`SECURITY.md`](SECURITY.md) instead
of opening a public issue.

## License

Released under `MIT OR Apache-2.0`. See [`LICENSE`](LICENSE) (MIT) and
[`LICENSE-APACHE`](LICENSE-APACHE).
