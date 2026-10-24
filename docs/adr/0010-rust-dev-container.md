# ADR-0010: The Dev Container adds the locked Rust Feature to the Debian base image, runs `scripts/install-tools.sh` after creation, and gives no access to containers

- **Status:** 🟡 proposed
- **Date:** 2026-10-23
- **Deciders:** Markus Brigl
- **Applies to:** `.devcontainer/devcontainer.json` and `.devcontainer/devcontainer-lock.json`

## Context

The template's container is `mcr.microsoft.com/devcontainers/base:debian` with the GitHub CLI and
no language toolchain; [ADR-0004](0004-dev-container-runtime.md) keeps the host's Docker socket
out and locks the Features. The toolchain of [ADR-0008](0008-rust-toolchain.md) needs rustup, the
toolchain pinned in `rust-toolchain.toml`, `cargo-deny`, and the `wasm-bindgen` CLI, and installs
the last three with `scripts/install-tools.sh`. Nothing the specification's goals test needs a
running service or a container. The harness ([ADR-0007](0007-harness.md)) keeps parallel sessions
apart as worktrees in one repository.

## Decision

We will add the Feature `ghcr.io/devcontainers/features/rust:1`, locked in
`devcontainer-lock.json`, to the template's base image for rustup, run `scripts/install-tools.sh`
in `postCreateCommand` for the pinned toolchain and tools, and give the container no access to any
container engine.

**Out of scope:** a container engine for an adapter whose tests need a database, decided with that
adapter; a container per session; Node and its tools.

## Alternatives considered

- **A custom `Dockerfile`** — full control of the image, but one more file to maintain and
  rebuild, for what a locked Feature and rustup already give.
- **The Feature's `version` set to the pinned toolchain** — saves one toolchain download, at the
  cost of a second place that names the version; `none` would skip rustup altogether.
- **Compose with services** — nothing needs a service yet.

## Sources / Prior art

- The Feature's definition and install script,
  <https://github.com/devcontainers/features/tree/main/src/rust> (version 1.6.0, checked
  2026-10-23): `version` defaults to `latest`; `none` installs no rustup.
- The catalogue entry *Container access* of the `propose-adr` skill.

## Consequences

- Positive: a new container builds the project without manual steps; the toolchain version lives
  in `rust-toolchain.toml` alone, used by the container, CI, and local runs alike.
- Negative / trade-offs: the Feature installs the latest stable beside the pinned toolchain, one
  unused toolchain in the image; `cargo install` compiles both tools on container creation, which
  takes minutes; parallel worktrees each compile the workspace into their own `target/`.
- Follow-ups: a shared build cache across worktrees, if build times hurt.

## Enforcement

`scripts/check-devcontainer.sh` ([ADR-0004](0004-dev-container-runtime.md)) fails on a host
socket, an unlocked Feature, or a Feature without a major version tag.

**Not mechanically decidable:** that the container builds the project is shown by building it, and
no CI job builds the container (ADR-0004); a Feature that would run an engine inside the container
is caught in review.
