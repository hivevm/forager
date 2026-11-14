# ADR-0007: The toolchain is pinned stable Rust with its cargo tools, the wasm-bindgen CLI, and the ShellCheck and jscpd release binaries, installed by `scripts/install-tools.sh` and run by the checks, with every sensor `scripts/check-all.sh` runs

- **Status:** 🟢 accepted
- **Date:** 2026-11-13
- **Deciders:** Markus Brigl
- **Applies to:** `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`, `crates/engine/clippy.toml`, `crates/label/clippy.toml`, `crates/symbolize/clippy.toml`, `rustfmt.toml`, `deny.toml`, `scripts/install-tools.sh`, `scripts/install-shellcheck.sh`, `scripts/install-jscpd.sh`, `scripts/check-rust.sh`, the ShellCheck of `scripts/check-shell.sh` and its self-test, every `scripts/sensor-*.sh` that `scripts/check-all.sh` runs and its self-test, `.jscpd.json`, the `postCreateCommand` of `.devcontainer/devcontainer.json`, the `rust`, `shell`, and `sensors` jobs of `.github/workflows/checks.yml`, the scheduled workflows of the slow sensors (`.github/workflows/fuzz.yml`, `.github/workflows/mutants.yml`), and the **Build, Test & Run** section of `README.md`

## Context

The specification fixes the language by its constraints: a core that builds for
`wasm32-unknown-unknown` without C code (Q-1), deterministic output (Q-2), no panics on user data,
measured by a weekly fuzz run (Q-3), a WASM bundle under 3 MB gzipped (Q-5), permissive licences
checked with `cargo deny` (Q-6), and `cargo fmt`, `cargo clippy -D warnings`, and all tests green
(*Constraints*). The crates its *Assumptions* lean on are Rust crates. The toolchain decision is
which version, which commands, which scripts, and which sensors.

The catalogue entry *Toolchain* of the `propose-adr` skill asks for the commands of the README,
one script under `scripts/` that runs them, and a tool or "none" per kind of sensor: behaviour,
maintainability, architecture fitness. Its table lists for Rust cargo-deny `bans`, clippy
`too_many_lines` and `cognitive_complexity`, rustc `dead_code`, jscpd for duplication, a diff rule
for ignored tests, and cargo-mutants. The entry also names this ADR as the one the review skill,
the pull request template, and the shape entries mean by "the sensors the toolchain ADR names".

The browser build needs the glue between WebAssembly and JavaScript. `wasm-bindgen` generates it;
`wasm-pack` wraps `cargo build` and the `wasm-bindgen` CLI and adds npm packaging. Both are
maintained under the `wasm-bindgen` organisation. The CLI must match the version of the
`wasm-bindgen` crate exactly.

[ADR-0002](0002-decisions-verified-by-tests.md) names ShellCheck as the one dependency of the
checks beyond bash and the base image's tools, and leaves open where it comes from. The Dev
Container once took Debian's package (0.10.0) and CI the newer one the runner image ships. A newer
ShellCheck brings new findings: pull request #170 failed its `shell` job on SC2329, which 0.11
reports where 0.10 reports SC2317, after `check-all.sh` and the pre-push hook had passed locally.
The runner image moves without a change in the repository. ShellCheck is written in Haskell, so
`cargo install` does not build it, and its source build needs a Haskell toolchain the project has
no other use for. Its maintainer publishes a statically linked binary per platform with each
release on GitHub (`koalaman/shellcheck`), with no checksum file beside it. GitHub records the
SHA-256 of every release asset at upload and shows it with the asset.

**Sensors named one by one.** The toolchain decision of 2026-11-10 named
`scripts/sensor-tests-kept.sh` and no other sensor script. `scripts/check-all.sh` runs eight more
today: `sensor-refactor-tests.sh`,
`sensor-binding-adrs.sh`, `sensor-plain-rules.sh`, `sensor-revision-due.sh`,
`sensor-plan-frontier.sh`, `sensor-backlog.sh`, `sensor-action-refs.sh` (with
`--require major-tag`, as the check of [ADR-0010](0010-actions-by-major-tag.md)), and
`sensor-review-binding.sh`. `sensor-repo-settings.sh` runs on a human's instruction only. The
review skill reads "the sensors the toolchain ADR names", and then lists two sensors by name. The
refactor tests sensor was not among those the decision of 2026-11-10 named. Each new sensor
therefore edited the skill, and one follow-up had to add a missing line (ticket 19 of the revision
of 2026-11-13).

**Duplication.** The decision of 2026-11-10 left duplication without a sensor, because jscpd needed
Node, and promised one "once a design revision finds duplication". The revision of 2026-11-13 found
it repeatedly: the drawing of a display item written five times before 3f56d73, a symbol's name
looked up twice before c6e9e1a, C's ports before ef639d7 and 3beee16, and tickets 03, 04, 07, 10,
11, and 18 of that revision. The maintainer chose on 2026-11-13 to add a duplication sensor with a
tool the environment carries without Node.

The research of 2026-11-13 found that the premise has moved. jscpd 5 is a Rust rewrite by jscpd's
own author: a self-contained binary without a Node runtime, on crates.io since 2026-06-05, 5.4.3
of 2026-11-08 the latest. It reads `rust` and `bash` among 224 formats. It is MIT-licensed. It
reports and exits 0 unless `--threshold` or `--exit-code` asks it to fail. It honours
`.gitignore`. Each GitHub release carries a static (musl) Linux binary for x86_64 and aarch64, a
`checksums.txt`, Sigstore signatures, and SLSA build provenance.

The crate is heavy to build: its own `Cargo.lock` holds 403 packages, among them `candle-core`,
`tokenizers`, `tree-sitter` with grammars in C, `ureq`, and `openssl-sys`, for its semantic and
language-server modes. `cargo install` builds all of them on every cache miss. The release binary
is about 10 MB and needs no compiler.

The other candidates, each checked on 2026-11-13:

| Tool | Rust | Bash | Install | Licence | Latest | Fit |
|---|---|---|---|---|---|---|
| jscpd 5 (`kucherenko/jscpd`) | yes | yes | release binary, `cargo install jscpd`, npm | MIT | 5.4.3, 2026-11-08 | token clones; report-only by default |
| PMD CPD | since PMD 7.9.0 | not listed | Java runtime | BSD-style | 7.29.0, 2026-10-30 | needs a JVM the container does not carry |
| Simian | not listed | not listed | JVM | Apache-2.0 per its site | not stated | needs a JVM; Rust unconfirmed |
| lizard `-Eduplicate` | yes (lizard) | no | pip; the container has Python 3.13 but no pip | MIT | 1.26.0, 2026-11-10 | the extension's Rust coverage undocumented; a third package ecosystem |
| similarity-rs (`mizchi/similarity`) | yes | no | `cargo install` | MIT | 0.5.0, 2026-04-11 | function-level AST similarity; marked beta, "not production-tested"; exit codes undocumented |
| cargo-dupes | yes | no | `cargo install` | MIT | 0.2.1, 2026-02-17 | `syn`-based; nine stars, last push 2026-03-06 |
| jscpd-rs (`vv-bogdanov`) | not stated | not stated | `cargo install` | MIT | 0.1.12, 2026-06-03 | a third party's clone, overtaken by upstream's own crate |
| polydup | yes | not stated | `cargo install` | MIT OR Apache-2.0 | 0.9.3, 2026-01-25 | 282 downloads in all |

The checks of [ADR-0002](0002-decisions-verified-by-tests.md) that the template ships need only
bash and the base image's tools, ShellCheck aside. The duplication sensor is the project's own, as
`scripts/check-rust.sh` is, so a tool of its own does not break that rule. It is still the second
binary the checks download, and it runs in the `sensors` job, which installed nothing before it.

Clippy reads the `clippy.toml` nearest a crate's manifest in place of the workspace's. The three
crates that port MapServer's drawing, `engine`, `label`, and `symbolize`, refuse `min` and `max`
of `f64` and `f32` through a `clippy.toml` of their own (4fba7fd), because those drop a NaN
operand silently, where `ms_min` and `ms_max` of [ADR-0017](0017-c-ports-in-style-ir.md) decide it
by the order of their operands. A copy that left out a ban of the workspace would lift that ban
for its crate.

## Decision

We will build, test, and lint with the pinned tools `scripts/install-tools.sh` installs, run
through `scripts/check-rust.sh`, `scripts/check-shell.sh`, and the sensors `scripts/check-all.sh`
runs:

- **Rust.** `rust-toolchain.toml` pins stable Rust, 1.99.1 at setup, with the components
  `rustfmt` and `clippy` and the target `wasm32-unknown-unknown`. The workspace uses edition 2024.
- **Tools.** `scripts/install-tools.sh` installs these, and skips a tool already at its version:
  - the toolchain of `rust-toolchain.toml`, with rustup;
  - `cargo-deny` at the version the script names, by `cargo install --locked`;
  - the `wasm-bindgen` CLI at the version of the `wasm-bindgen` crate in `Cargo.lock`, by
    `cargo install --locked`;
  - ShellCheck, through `scripts/install-shellcheck.sh`;
  - jscpd, through `scripts/install-jscpd.sh`.
- **Where it runs.** The Dev Container runs `scripts/install-tools.sh` in its `postCreateCommand`.
  The `rust` job of `checks.yml` runs it too, and caches `~/.cargo/bin` with `actions/cache`.
- **ShellCheck.** The shell lint, locally and in CI, must run ShellCheck's release binary of the
  version `scripts/install-shellcheck.sh` names, 0.11.0:
  - the script downloads it from the `koalaman/shellcheck` release on GitHub;
  - the script refuses the download unless its SHA-256 matches the digest it pins per platform;
  - the `shell` job of `checks.yml` runs the script in a step before the lint;
  - `scripts/check-shell.sh` names the running and the pinned version where they differ, and
    fails in a CI run.
- **jscpd.** The duplication sensor, locally and in CI, must run jscpd's static Linux release
  binary of the version `scripts/install-jscpd.sh` names, 5.4.3:
  - the script downloads the `linux-x64-musl` or `linux-arm64-musl` archive from the
    `kucherenko/jscpd` release on GitHub;
  - the script refuses the download unless its SHA-256 matches the digest it pins per platform;
  - the `sensors` job of `checks.yml` runs the script in a step before the sensor.
- **Commands.** `scripts/check-rust.sh` runs every build, test, and lint command of the workspace.
  `scripts/check-all.sh` invokes it, and the `rust` job of `checks.yml` runs it. It must run
  these in this order:
  1. `cargo fmt --all --check`;
  2. `cargo clippy --workspace --all-targets -- -D warnings`, with `clippy::too_many_lines` and
     `clippy::cognitive_complexity` warned in the workspace lints;
  3. `cargo test --workspace`;
  4. `cargo build` for `wasm32-unknown-unknown` of every crate that must run in the browser;
  5. `wasm-bindgen --target web` on the browser bindings, and a check of the gzipped `.wasm`
     against Q-5;
  6. `cargo deny check`.

  Other ADRs add steps of their own to the script.
- **Clippy's configuration.** `clippy.toml` holds the workspace's lists. The `clippy.toml` of
  `crates/engine`, `crates/label`, and `crates/symbolize` must hold the same lists, with the bans
  of `min` and `max` of `f64` and `f32` added. No other crate carries a `clippy.toml` of its own.
- **Sensors.** The sensors of this ADR are the slow ones below and every `scripts/sensor-*.sh`
  that `scripts/check-all.sh` runs on a `run` line. A sensor that runs only on a human's
  instruction is none of them. By kind:
  - Behaviour: `cargo test`, whose tests cite goals and ADRs with `// Verifies:`.
  - Maintainability: the clippy lints above and rustc's `dead_code`, made errors by
    `-D warnings`, and the sensor scripts above, the tests-kept sensor among them.
  - Duplication: `scripts/sensor-duplication.sh` runs jscpd on the tracked Rust and bash files.
    It reports each clone with its two places, and fails on no count.
    `.jscpd.json` holds its formats, paths, and minimum clone size.
  - Architecture fitness: the dependency test of [ADR-0008](0008-ports-and-adapters-crates.md).
  - Slow, in scheduled workflows of their own: `cargo-mutants` in `mutants.yml`, and the fuzz
    targets of Q-3 with `cargo-fuzz` in `fuzz.yml`, on a nightly toolchain installed for that run.

**Out of scope:**

- the crates the code depends on, each chosen by the ADR that first needs it;
- releasing crates;
- how the wasm32 renders of G-9 run, which
  [ADR-0034](0034-wasm-renders-compared-in-wasmi.md) decides;
- a platform other than Linux on x86_64 or aarch64, where `scripts/install-shellcheck.sh` and
  `scripts/install-jscpd.sh` fail and name the platform;
- the other tools of the base image, which [ADR-0002](0002-decisions-verified-by-tests.md) leaves
  to it;
- when the ShellCheck or jscpd pin moves. A move changes the script's version and digests, and
  fixes the findings of the new version in the same change;
- what each sensor reports, which the ADR or change that adds it decides;
- turning the duplication sensor into a check that fails, by `--threshold` or a baseline, which
  a later decision takes once its reports have been read;
- jscpd's semantic, language-server, and MCP modes.

## Alternatives considered

- **wasm-pack** — maintained and convenient; it wraps the same two steps and adds npm packaging
  and a `package.json` the project has no use for. One tool fewer to pin and install.
- **Prebuilt release binaries of the cargo tools, fetched in the script** — faster than
  `cargo install`, but each needs a checksum kept by hand; the cache makes `cargo install` a
  one-time cost. ShellCheck is the exception, since `cargo install` does not build it, and so is
  jscpd, whose build is no small one-time cost (below).
- **A third-party action to install Rust or the tools** — rustup reads `rust-toolchain.toml`
  itself, and the workflows use the `actions/*` the template ships
  ([ADR-0010](0010-actions-by-major-tag.md)).
- **Unpinned stable** — a new compiler or clippy release would turn a green commit red without a
  change; the pin moves deliberately.
- **Commands inline in the workflow** — they would run in CI and never in the pre-push hook; the
  catalogue's *Does not fit*.
- **Debian's ShellCheck in the Dev Container, the runner image's in CI** — the state before the
  decision of 2026-11-10: two versions that differ and move on their own, the failure of #170.
- **Debian's ShellCheck in both places** — the runner is Ubuntu, whose package lags Debian's and
  differs from it; one version in both places would still not hold.
- **`check-shell.sh` failing on another version, installing nothing** — the least change, but the
  lint stays red until someone installs the right version by hand, in CI on every runner image
  update.
- **Building ShellCheck from source** — a Haskell toolchain in the container and in CI for one
  lint.
- **A container image or GitHub Action that ships ShellCheck** — a second supply chain to trust,
  and the Dev Container would still need its own install.
- **Naming each sensor script in this ADR** — what the decision of 2026-11-10 chose, with
  `scripts/sensor-tests-kept.sh` the one script it named. Every new sensor then supersedes the
  toolchain ADR and edits the review skill; that decision already missed the refactor tests
  sensor.
- **Naming the sensors by every `scripts/sensor-*.sh` in the tree** — it would take in
  `sensor-repo-settings.sh`, which reads GitHub and runs on a human's instruction only, so no
  review has its output.
- **No duplication sensor, the review alone** — what the decision of 2026-11-10 chose, since jscpd
  then needed Node, which the environment does not carry. It promised a sensor once a design
  revision finds duplication. The revision of 2026-11-13 found duplication the reviews had let
  through, again and again.
- **`cargo install --locked jscpd`** — the install path of the other cargo tools, and the crate is
  upstream's own. Its lock builds 403 packages, C grammars and `openssl-sys` among them, on every
  cache miss, in the container and in the `sensors` job. The release binary needs a pinned digest,
  as ShellCheck's does, and upstream also signs it.
- **jscpd 4 through npm** — needs Node, which [ADR-0009](0009-dev-container.md) leaves out of the
  container.
- **PMD CPD** — reads Rust since PMD 7.9.0, and is maintained; it needs a Java runtime in the
  container and in CI for one sensor.
- **Simian** — needs a JVM, and its site lists neither Rust nor shell.
- **lizard with `-Eduplicate`** — reads Rust, is maintained, and comes from PyPI. The container
  has Python 3.13 without pip, so it adds pip and a third package ecosystem. Its documentation
  does not say which languages the duplicate extension covers, and lizard reads no bash.
- **similarity-rs** — compares functions by their syntax trees, which finds renamed copies. It
  reads Rust alone, its authors mark it beta and "not production-tested", and its exit codes are
  undocumented.
- **cargo-dupes** — normalizes `syn` trees and has a report and a check mode. It reads Rust alone,
  is a few months old with nine stars, and saw no push since 2026-03-06.
- **jscpd-rs and polydup** — a third party's clone of jscpd that upstream's own crate overtook,
  and a tool with 282 downloads in all.

## Sources / Prior art

- Rust release channel, <https://static.rust-lang.org/dist/channel-rust-stable.toml>: 1.99.1,
  2026-10-13.
- crates.io, checked 2026-10-23: `wasm-bindgen` 0.2.129, `wasm-pack` 0.15.0, `cargo-deny` 0.20.2,
  `cargo-mutants` 27.1.0.
- cargo-fuzz requires a nightly toolchain: <https://rust-fuzz.github.io/book/cargo-fuzz.html>.
- The catalogue entry *Toolchain* of the `propose-adr` skill and its table of sensors.
- ShellCheck releases, `https://github.com/koalaman/shellcheck/releases/tag/v0.11.0`, and the
  asset digests GitHub's REST API returns for it
  (`https://api.github.com/repos/koalaman/shellcheck/releases/tags/v0.11.0`), read 2026-11-10:
  `linux.x86_64.tar.xz` has `8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198`,
  `linux.aarch64.tar.xz` `12b331c1d2db6b9eb13cfca64306b1b157a86eb69db83023e261eaa7e7c14588`.
- ShellCheck's changelog for v0.11.0: SC2329, warn when (non-escaping) functions are never invoked.
- GitHub, *About releases*, on the digests of release assets,
  `https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases`.
- The design revision of 2026-11-13, `docs/plan/revision-2026-11-13/19-adr-0007-sensors.md`, and
  the run lines of `scripts/check-all.sh` at 69c845a.
- jscpd, read 2026-11-13:
  - README, <https://github.com/kucherenko/jscpd> and <https://unpkg.com/jscpd@5.3.0/README.md>:
    "jscpd v5 is the Rust engine — a self-contained binary, no Node.js runtime"; install by
    release binary, `cargo install jscpd`, or npm; `--min-tokens` 50 and `--min-lines` 5 by
    default; `--threshold`; `.jscpd.json`; `--changed` and `--baseline-from-ref`.
  - Formats, <https://github.com/kucherenko/jscpd/blob/master/FORMATS.md>: `rust` for `.rs`,
    `bash` for `.sh`, `.bash`, and scripts with a shell shebang.
  - CLI source, `rust/crates/cpd/src/cli.rs`: `--exit-code` ("Exit with code if duplicates
    found"), `--threshold` ("Maximum duplication percentage before exit 1"), `--no-gitignore`.
  - Release v5.4.3, 2026-11-08, via
    `https://api.github.com/repos/kucherenko/jscpd/releases/latest`: `jscpd-linux-x64-musl.tar.gz`
    has `291452d6005e3fa17b531552bf76f87ff85bc4ae31533e0c17a7997f84079868`,
    `jscpd-linux-arm64-musl.tar.gz`
    `f363875348817f8e999ca090266b1265ba29322d6b5d1f6280aa0d136ab1ca9e`; `checksums.txt`, a
    `.sigstore.json` per asset, and `multiple.intoto.jsonl` beside them. The repository: MIT,
    about 6,500 stars, last push 2026-11-13.
  - crates.io, `https://crates.io/api/v1/crates/jscpd`: 5.4.3, MIT, MSRV 1.97, owner
    `kucherenko`, first published 2026-06-05; the packaged `Cargo.lock` of 5.4.3 holds 403
    packages.
- PMD, *Rust* language page, <https://docs.pmd-code.org/latest/pmd_languages_rust.html>: CPD
  support since 7.9.0, no rules; *Copy/Paste Detector*,
  <https://docs.pmd-code.org/latest/pmd_userdocs_cpd.html>: PMD 7.29.0 of 2026-10-30.
- Simian, <https://simian.quandarypeak.com/>: Apache-2.0, JVM, its list of languages.
- lizard, <https://github.com/terryyin/lizard>, its extensions' README
  `lizard_ext/README.md`, and PyPI `https://pypi.org/pypi/lizard/json`: 1.26.0 of 2026-11-10,
  MIT, Rust among its languages, no shell.
- similarity-rs, <https://github.com/mizchi/similarity>, and crates.io: 0.5.0 of 2026-04-11, MIT,
  beta.
- cargo-dupes, <https://github.com/mpecan/cargo-dupes>, and crates.io: 0.2.1 of 2026-02-17, MIT.
- jscpd-rs (crates.io, 0.1.12 of 2026-06-03) and polydup (crates.io, 0.9.3 of 2026-01-25).

## Consequences

- Positive: one command reproduces CI locally; the WASM build and its size are checked on every
  pull request, so a dependency that breaks Q-1 or Q-5 fails where it is added. Tool versions live
  in the install scripts and in `Cargo.lock`.
- Positive: the pre-push hook and CI lint with the same ShellCheck, so a runner image update no
  longer changes the gate. The binary is checked against a digest in the repository, so a
  replaced asset fails the install.
- Positive: a new sensor joins this ADR by its `run` line in `scripts/check-all.sh`, without a
  supersession. The review skill's line holds without an edit: it reads "the output of the
  sensors the toolchain ADR names, every `scripts/sensor-*.sh` that `scripts/check-all.sh` runs,
  as `scripts/check-all.sh --verbose` prints it".
- Positive: the reviewer sees duplicated Rust and bash with both places named, which the revision
  of 2026-11-13 found only after the merge. One tool reads both languages.
- Negative / trade-offs: the `wasm-bindgen` CLI moves with the crate, so a crate update rebuilds
  the CLI; the first CI run and every cache miss spend minutes in `cargo install`. Building for two
  targets lengthens the job. A panic the fuzzers find surfaces up to a week after the change that
  caused it.
- Negative / trade-offs: the ShellCheck digest is GitHub's, recorded at upload, not a signature of
  ShellCheck's maintainer; the trust is in the release as uploaded. The `shell` job and every
  container creation download about 2.5 MB. A container created before ShellCheck came from the
  release keeps Debian's package in `/usr/bin`, ahead of `~/.local/bin` on `PATH`, until it is
  rebuilt; the install script says so, and the lint warns of the version.
- Negative / trade-offs: jscpd is a binary the project did not build, about 10 MB per container
  and per `sensors` run. Its 403 crates stay outside `Cargo.lock`, so `cargo deny` of
  [ADR-0011](0011-crate-supply-chain.md) checks none of them, and `deny.toml` does not change. The
  same holds today for `cargo-deny` and the `wasm-bindgen` CLI. The trust is in the release as
  uploaded; upstream's Sigstore signature and SLSA provenance could back the pinned digest.
- Negative / trade-offs: jscpd matches tokens, so it finds copies and renamed copies, not the same
  logic written twice in a different shape; the review still judges those. Its release cadence
  is weekly, so the pin falls behind quickly. A whole-tree report lists the clones that predate a
  change.
- Negative / trade-offs: [ADR-0009](0009-dev-container.md) does not change; the container gains
  no Feature and no Node, and its `postCreateCommand` already runs `scripts/install-tools.sh`.
- Follow-ups:
  - the catalogue entry *Toolchain*, whose Rust row names jscpd without saying it no longer needs
    Node, in the template;
  - moving the ShellCheck and jscpd pins, their digests, and the findings a new version brings,
    together;
  - whether the duplication sensor reports only the clones a change adds or touches, with
    jscpd's `--changed` or `--baseline-from-ref`;
  - a check that fails on duplication, once the reports show a threshold worth holding.

## Enforcement

- The golden path's tests cite this ADR with a `Verifies:` marker.
- Check 9 of `scripts/check-docs.sh` holds the `rust` and `sensors` jobs, the `run` lines of
  `scripts/check-all.sh`, and the required checks of `CONTRIBUTING.md` together. The sensors this
  ADR names are those `run` lines, so check 9 holds that list too.
- `scripts/test-check-shell.sh` decides that a ShellCheck of another version than the pinned is
  named with it and fails the lint in a CI run. It cites this ADR with `Verifies:`. The `shell` job
  runs it before the lint, and the lint after the install step.
- `scripts/install-shellcheck.sh` and `scripts/install-jscpd.sh` fail on a digest that does not
  match, through `sha256sum --check`.
- `scripts/test-sensor-duplication.sh` decides that the sensor names both places of a clone in a
  fixture of Rust and of bash, and exits 0 with clones found. It cites this ADR with `Verifies:`.
  The `sensors` job runs it before the sensor, after the install step.
- `the_crates_that_port_mapservers_drawing_ban_what_the_workspace_bans` in
  `crates/arch-test/tests/dependencies.rs` decides that the `clippy.toml` of `engine`, `label`, and
  `symbolize` holds the workspace's lists with the min and max bans added, and that no other
  crate carries one. It cites this ADR with `Verifies:`.
