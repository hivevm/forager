# ADR-0007: The toolchain is pinned stable Rust with its cargo tools, the wasm-bindgen CLI, and one ShellCheck release binary, installed by `scripts/install-tools.sh` and run by the checks

- **Status:** 🟢 accepted
- **Date:** 2026-11-10
- **Deciders:** Markus Brigl
- **Applies to:** `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `scripts/install-tools.sh`, `scripts/install-shellcheck.sh`, `scripts/check-rust.sh`, the ShellCheck of `scripts/check-shell.sh` and its self-test, the `postCreateCommand` of `.devcontainer/devcontainer.json`, the `rust` and `shell` jobs of `.github/workflows/checks.yml`, the scheduled workflows of the slow sensors (`.github/workflows/fuzz.yml`, `.github/workflows/mutants.yml`), and the **Build, Test & Run** section of `README.md`

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
for ignored tests, and cargo-mutants.

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

## Decision

We will build, test, and lint with the pinned tools `scripts/install-tools.sh` installs, run
through `scripts/check-rust.sh` and `scripts/check-shell.sh`:

- **Rust.** `rust-toolchain.toml` pins stable Rust, 1.99.1 at setup, with the components
  `rustfmt` and `clippy` and the target `wasm32-unknown-unknown`. The workspace uses edition 2024.
- **Tools.** `scripts/install-tools.sh` installs these, and skips a tool already at its version:
  - the toolchain of `rust-toolchain.toml`, with rustup;
  - `cargo-deny` at the version the script names, by `cargo install --locked`;
  - the `wasm-bindgen` CLI at the version of the `wasm-bindgen` crate in `Cargo.lock`, by
    `cargo install --locked`;
  - ShellCheck, through `scripts/install-shellcheck.sh`.
- **Where it runs.** The Dev Container runs `scripts/install-tools.sh` in its `postCreateCommand`.
  The `rust` job of `checks.yml` runs it too, and caches `~/.cargo/bin` with `actions/cache`.
- **ShellCheck.** The shell lint, locally and in CI, must run ShellCheck's release binary of the
  version `scripts/install-shellcheck.sh` names, 0.11.0:
  - the script downloads it from the `koalaman/shellcheck` release on GitHub;
  - the script refuses the download unless its SHA-256 matches the digest it pins per platform;
  - the `shell` job of `checks.yml` runs the script in a step before the lint;
  - `scripts/check-shell.sh` names the running and the pinned version where they differ, and
    fails in a CI run.
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
- **Sensors.**
  - Behaviour: `cargo test`, whose tests cite goals and ADRs with `// Verifies:`.
  - Maintainability: the clippy lints above and rustc's `dead_code`, made errors by
    `-D warnings`, and skipped tests through `scripts/sensor-tests-kept.sh`. Duplication has no
    sensor: jscpd needs Node, which the environment does not carry.
  - Architecture fitness: the dependency test of [ADR-0008](0008-ports-and-adapters-crates.md).
  - Slow, in scheduled workflows of their own: `cargo-mutants` in `mutants.yml`, and the fuzz
    targets of Q-3 with `cargo-fuzz` in `fuzz.yml`, on a nightly toolchain installed for that run.

**Out of scope:**

- the crates the code depends on, each chosen by the ADR that first needs it;
- releasing crates;
- how the wasm32 renders of G-9 run, which
  [ADR-0034](0034-wasm-renders-compared-in-wasmi.md) decides;
- a platform other than Linux on x86_64 or aarch64, where `scripts/install-shellcheck.sh` fails
  and names the platform;
- the other tools of the base image, which [ADR-0002](0002-decisions-verified-by-tests.md) leaves
  to it;
- when the ShellCheck pin moves. A move changes the script's version and digests, and fixes the
  findings of the new version in the same change.

## Alternatives considered

- **wasm-pack** — maintained and convenient; it wraps the same two steps and adds npm packaging
  and a `package.json` the project has no use for. One tool fewer to pin and install.
- **Prebuilt release binaries of the cargo tools, fetched in the script** — faster than
  `cargo install`, but each needs a checksum kept by hand; the cache makes `cargo install` a
  one-time cost. ShellCheck is the exception, since `cargo install` does not build it.
- **A third-party action to install Rust or the tools** — rustup reads `rust-toolchain.toml`
  itself, and the workflows use the `actions/*` the template ships
  ([ADR-0010](0010-actions-by-major-tag.md)).
- **Unpinned stable** — a new compiler or clippy release would turn a green commit red without a
  change; the pin moves deliberately.
- **Commands inline in the workflow** — they would run in CI and never in the pre-push hook; the
  catalogue's *Does not fit*.
- **Debian's ShellCheck in the Dev Container, the runner image's in CI** — the state before this
  decision: two versions that differ and move on their own, the failure of #170.
- **Debian's ShellCheck in both places** — the runner is Ubuntu, whose package lags Debian's and
  differs from it; one version in both places would still not hold.
- **`check-shell.sh` failing on another version, installing nothing** — the least change, but the
  lint stays red until someone installs the right version by hand, in CI on every runner image
  update.
- **Building ShellCheck from source** — a Haskell toolchain in the container and in CI for one
  lint.
- **A container image or GitHub Action that ships ShellCheck** — a second supply chain to trust,
  and the Dev Container would still need its own install.

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

## Consequences

- Positive: one command reproduces CI locally; the WASM build and its size are checked on every
  pull request, so a dependency that breaks Q-1 or Q-5 fails where it is added. Tool versions live
  in the install scripts and in `Cargo.lock`.
- Positive: the pre-push hook and CI lint with the same ShellCheck, so a runner image update no
  longer changes the gate. The binary is checked against a digest in the repository, so a
  replaced asset fails the install.
- Negative / trade-offs: the `wasm-bindgen` CLI moves with the crate, so a crate update rebuilds
  the CLI; the first CI run and every cache miss spend minutes in `cargo install`. Building for two
  targets lengthens the job. A panic the fuzzers find surfaces up to a week after the change that
  caused it.
- Negative / trade-offs: the ShellCheck digest is GitHub's, recorded at upload, not a signature of
  ShellCheck's maintainer; the trust is in the release as uploaded. The `shell` job and every
  container creation download about 2.5 MB. A container created before ShellCheck came from the
  release keeps Debian's package in `/usr/bin`, ahead of `~/.local/bin` on `PATH`, until it is
  rebuilt; the install script says so, and the lint warns of the version.
- Follow-ups: a duplication sensor once a design revision finds duplication; moving the ShellCheck
  pin, its digests, and the findings a new version brings, together.

## Enforcement

- The golden path's tests cite this ADR with a `Verifies:` marker.
- Check 9 of `scripts/check-docs.sh` holds the `rust` job, the `run` line of
  `scripts/check-all.sh`, and the required checks of `CONTRIBUTING.md` together.
- `scripts/test-check-shell.sh` decides that a ShellCheck of another version than the pinned is
  named with it and fails the lint in a CI run. It cites this ADR with `Verifies:`. The `shell` job
  runs it before the lint, and the lint after the install step.
- `scripts/install-shellcheck.sh` fails on a digest that does not match, through
  `sha256sum --check`.
