# ADR-0008: The toolchain is pinned stable Rust with cargo, clippy, rustfmt, cargo-deny, and the wasm-bindgen CLI, installed by `scripts/install-tools.sh` and run by `scripts/check-rust.sh`

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `scripts/install-tools.sh`, `scripts/check-rust.sh`, the `rust` job of `.github/workflows/checks.yml`, the scheduled workflow of the slow sensors, and the **Build, Test & Run** section of `README.md`

## Context

The specification fixes the language by its constraints: a core that builds for
`wasm32-unknown-unknown` without C code (Q-1), deterministic output (Q-2), no panics on user data,
measured by a weekly fuzz run (Q-3), a WASM bundle under 3 MB gzipped (Q-5), permissive licences
checked with `cargo deny` (Q-6), and `cargo fmt`, `cargo clippy -D warnings`, and all tests green
(*Constraints*). The crates its *Assumptions* lean on are Rust crates. The toolchain decision is
which version, which commands, which scripts, and which sensors.

The catalogue entry *Toolchain* asks for the commands of the README, one script under `scripts/`
that runs them, and a tool or "none" per kind of sensor: behaviour, maintainability, architecture
fitness. Its table lists for Rust cargo-deny `bans`, clippy `too_many_lines` and
`cognitive_complexity`, rustc `dead_code`, jscpd for duplication, a diff rule for ignored tests,
and cargo-mutants.

The browser build needs the glue between WebAssembly and JavaScript. `wasm-bindgen` generates it;
`wasm-pack` wraps `cargo build` and the `wasm-bindgen` CLI and adds npm packaging. Both are
maintained under the `wasm-bindgen` organisation. The CLI must match the version of the
`wasm-bindgen` crate exactly.

## Decision

We will build with stable Rust pinned in `rust-toolchain.toml` (1.99.1 at setup, edition 2024,
target `wasm32-unknown-unknown`, components `rustfmt` and `clippy`), install the extra tools with
`scripts/install-tools.sh`, and run every build, test, and lint command through
`scripts/check-rust.sh`, which `scripts/check-all.sh` invokes and the `rust` job of `checks.yml`
runs:

- **Tools.** `scripts/install-tools.sh` installs the toolchain of `rust-toolchain.toml` with
  rustup, then `wasm-bindgen-cli` at the version of the `wasm-bindgen` crate in `Cargo.lock` and
  `cargo-deny` at the version the script names, each by `cargo install --locked`, skipping a tool
  already at its version. The Dev Container and CI both run it; CI caches `~/.cargo/bin` with
  `actions/cache`.
- **Commands,** in this order: `cargo fmt --all --check`;
  `cargo clippy --workspace --all-targets -- -D warnings`, with `clippy::too_many_lines` and
  `clippy::cognitive_complexity` warned in the workspace lints; `cargo test --workspace`;
  `cargo build` of every crate that must run in the browser for `wasm32-unknown-unknown`;
  `wasm-bindgen --target web` on the browser bindings and a check of the gzipped `.wasm` against
  Q-5; `cargo deny check`.
- **Sensors.** Behaviour: `cargo test`, its tests citing goals and ADRs with `// Verifies:`.
  Maintainability: the clippy lints above and rustc's `dead_code`, made errors by `-D warnings`;
  skipped tests through `scripts/sensor-tests-kept.sh`; duplication: none yet, since jscpd needs
  Node, which the environment does not carry. Architecture fitness: the dependency test of
  [ADR-0009](0009-ports-and-adapters-crates.md). Slow, in a scheduled workflow of their own:
  `cargo-mutants`, and the fuzz targets of Q-3 with `cargo-fuzz` on a nightly toolchain that
  workflow installs.

**Out of scope:** the crates the code depends on, each chosen by the ADR that first needs it;
releasing crates; the browser test runner of G-9.

## Alternatives considered

- **wasm-pack** — maintained and convenient; it wraps the same two steps and adds npm packaging
  and a `package.json` the project has no use for. One tool fewer to pin and install.
- **Prebuilt release binaries fetched in the script** — faster than `cargo install`, but each
  needs a checksum kept by hand; the cache makes `cargo install` a one-time cost.
- **A third-party action to install Rust or the tools** — rustup reads `rust-toolchain.toml`
  itself; [ADR-0011](0011-action-references-major-tag.md) keeps the workflows to `actions/*`.
- **Unpinned stable** — a new compiler or clippy release would turn a green commit red without a
  change; the pin moves deliberately.
- **Commands inline in the workflow** — they would run in CI and never in the pre-push hook; the
  catalogue's *Does not fit*.

## Sources / Prior art

- Rust release channel, <https://static.rust-lang.org/dist/channel-rust-stable.toml>: 1.99.1,
  2026-10-13.
- crates.io, checked 2026-10-23: `wasm-bindgen` 0.2.129, `wasm-pack` 0.15.0, `cargo-deny` 0.20.2,
  `cargo-mutants` 27.1.0.
- cargo-fuzz requires a nightly toolchain: <https://rust-fuzz.github.io/book/cargo-fuzz.html>.
- The catalogue entry *Toolchain* of the `propose-adr` skill and its table of sensors.

## Consequences

- Positive: one command reproduces CI locally; the WASM build and its size are checked on every
  pull request, so a dependency that breaks Q-1 or Q-5 fails where it is added. Tool versions live
  in one script and in `Cargo.lock`.
- Negative / trade-offs: the `wasm-bindgen` CLI moves with the crate, so a crate update rebuilds
  the CLI; the first CI run and every cache miss spend minutes in `cargo install`. Building for two
  targets lengthens the job. A panic the fuzzers find surfaces up to a week after the change that
  caused it.
- Follow-ups: a duplication sensor once a design revision finds duplication; the browser test
  runner of G-9.

## Enforcement

The golden path's tests cite this ADR with a `Verifies:` marker, and it flips to accepted with
them. Check 9 of `scripts/check-docs.sh` holds the `rust` job, the `run` line of
`scripts/check-all.sh`, and the required checks of the README together.
