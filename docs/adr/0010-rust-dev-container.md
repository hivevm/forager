# ADR-0010: The Dev Container adds the locked Rust Feature to the Debian base image, installs the pinned cargo tools after creation, and gives no access to containers

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `.devcontainer/devcontainer.json`, `.devcontainer/devcontainer-lock.json`, and the worktrees of parallel sessions under `.scratch/`

## Context

The template's container is `mcr.microsoft.com/devcontainers/base:debian` with the GitHub CLI and
no language toolchain; ADR-0004 keeps the host's Docker socket out and locks the Features. The
toolchain of ADR-0008 needs `rustup` with the pinned toolchain, the `wasm32-unknown-unknown`
target, `cargo-deny`, and the `wasm-bindgen` CLI. Nothing Forager tests needs a running service
until a PostGIS adapter (phase 8). The harness (ADR-0007) runs up to three sessions in parallel,
each in a worktree.

## Decision

We will add the Feature `ghcr.io/devcontainers/features/rust:1`, locked in
`devcontainer-lock.json`, to the template's base image; let `rust-toolchain.toml` select the exact
toolchain and target through rustup; install `cargo-deny` and `wasm-bindgen-cli` in
`postCreateCommand` with `cargo install --locked` at the versions ADR-0008 pins; give the
container no access to any container engine; and keep parallel sessions apart as worktrees under
`.scratch/` in the one container, each with its own `target/`.

**Out of scope:** a container engine for the PostGIS adapter, decided with that adapter; a
container per session; Node and its tools.

## Alternatives considered

- **A custom `Dockerfile`** — full control of the image, but one more file to maintain and
  rebuild, for what a locked Feature and rustup already give.
- **Compose with services** — nothing needs a service yet.
- **A container per session** — stronger isolation than three worktrees need; costlier to start.

## Sources / Prior art

- The Feature's definition,
  <https://github.com/devcontainers/features/tree/main/src/rust> (version 1.6.0, checked
  2026-10-23).
- The catalogue entry *Container access* of the `propose-adr` skill.

## Consequences

- Positive: a new container builds the project without manual steps; the toolchain version lives
  in one file, `rust-toolchain.toml`, used by the container and by CI alike.
- Negative / trade-offs: `cargo install` compiles both tools on every container creation, which
  takes minutes; three worktrees each compile the workspace into their own `target/`.
- Follow-ups: a shared build cache across worktrees, if build times hurt.

## Enforcement

[`scripts/check-devcontainer.sh`](../../scripts/check-devcontainer.sh) (ADR-0004) fails on a host
socket, an unlocked Feature, or a Feature without a major version tag. A test in that script's
self-test, or a check beside it, that names the Rust Feature in `devcontainer.json` and the two
`cargo install --locked` lines cites this ADR with `Verifies:`.
