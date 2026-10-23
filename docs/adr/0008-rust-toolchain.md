# ADR-0008: The toolchain is pinned stable Rust with cargo, clippy, rustfmt, cargo-deny, and the wasm-bindgen CLI, run by `scripts/check-rust.sh`

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.cargo/`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `scripts/check-rust.sh`, the `rust` job of `.github/workflows/checks.yml`, the scheduled workflow of the slow sensors, and the **Build, Test & Run** section of `README.md`

## Context

The specification fixes the language by its constraints: a core that builds for
`wasm32-unknown-unknown` without C code (Q-1), deterministic output (Q-2), no panics on user data
(Q-3), a WASM bundle under 3 MB gzipped (Q-5), permissive licences checked with `cargo deny`
(Q-6), and `cargo fmt`, `cargo clippy -D warnings`, and all tests green. The brief names Rust and
its crates; the toolchain decision is which version, which commands, which script, and which
sensors.

The catalogue entry *Toolchain* asks for the commands of the README, one script under `scripts/`
that runs them, and a tool or "none" per kind of sensor: behaviour, maintainability, architecture
fitness. Its table lists for Rust cargo-deny `bans`, clippy `too_many_lines` and
`cognitive_complexity`, rustc `dead_code`, jscpd for duplication, a diff rule for `#[ignore]`, and
cargo-mutants.

The browser build needs the glue between WebAssembly and JavaScript. `wasm-bindgen` generates it;
`wasm-pack` wraps `cargo build` and the `wasm-bindgen` CLI and adds npm packaging. Both are
maintained under the `wasm-bindgen` organisation (wasm-bindgen 0.2.129, September 2026;
wasm-pack 0.15.0, May 2026).

## Decision

We will build with stable Rust pinned in `rust-toolchain.toml` (1.99.1 at setup, edition 2024,
target `wasm32-unknown-unknown`, components `rustfmt` and `clippy`), and run every build, test, and
lint command through one script, `scripts/check-rust.sh`, which `scripts/check-all.sh` invokes and
the `rust` job of `checks.yml` runs:

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`, with `clippy::too_many_lines` and
   `clippy::cognitive_complexity` warned in the workspace lints
3. `cargo test --workspace`
4. `cargo build` of every core crate and the browser bindings for `wasm32-unknown-unknown`
5. `wasm-bindgen --target web` on the browser bindings, with the CLI at exactly the version of the
   `wasm-bindgen` crate in `Cargo.lock`, and a size check of the gzipped `.wasm` against Q-5
6. `cargo deny check`

- **Sensors.** Behaviour: `cargo test`, its tests citing goals and ADRs with `// Verifies:`.
  Maintainability: the clippy lints above and rustc's `dead_code`, made errors by `-D warnings`;
  skipped tests through `scripts/sensor-tests-kept.sh`; duplication: none yet, since jscpd needs
  Node, which the environment does not carry — the design revision adds it when duplication shows
  up. Architecture fitness: `cargo deny` `bans` and the dependency test of the shape ADR. Slow,
  in a scheduled workflow of their own: `cargo-mutants` and the fuzz targets of Q-3 with
  `cargo-fuzz`.

**Out of scope:** the crates the code depends on, each chosen by the ADR that first needs it; the
release of crates to crates.io; the browser test runner of G-9.

## Alternatives considered

- **wasm-pack** — maintained, and convenient; it wraps the same two steps and adds npm packaging
  and a `package.json` the project has no use for. One tool fewer to pin and install.
- **Unpinned stable** — a new compiler or clippy release would turn a green commit red without a
  change; the pin moves deliberately.
- **Nightly** — needed by none of the brief's crates; `cargo-fuzz` brings its own nightly in the
  scheduled workflow.
- **Commands inline in the workflow** — they would run in CI and never in the pre-push hook; the
  catalogue's *Does not fit*.

## Sources / Prior art

- Rust release channel, <https://static.rust-lang.org/dist/channel-rust-stable.toml>: 1.99.1,
  2026-10-13.
- crates.io: `wasm-bindgen` 0.2.129 and `wasm-pack` 0.15.0 (repository
  <https://github.com/wasm-bindgen/wasm-pack>), `cargo-deny` 0.20.2, `cargo-mutants` 27.1.0,
  checked 2026-10-23.
- The catalogue entry *Toolchain* of the `propose-adr` skill and its table of sensors.

## Consequences

- Positive: one command reproduces CI locally; the WASM build and its size are checked on every
  pull request, so a dependency that breaks Q-1 or Q-5 fails where it is added.
- Negative / trade-offs: the `wasm-bindgen` CLI must move in lockstep with the crate; a version
  mismatch fails the build until both move. Building for two targets lengthens the job.
- Follow-ups: a duplication sensor once the design revision finds duplication; the browser test
  runner for G-9.

## Enforcement

The golden path's tests cite this ADR with a `Verifies:` marker, and it flips to accepted with
them. Check 9 of [`scripts/check-docs.sh`](../../scripts/check-docs.sh) holds the `rust` job, the
`run` line of `scripts/check-all.sh`, and the required checks of the README together.
