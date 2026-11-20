# OPS-dev-container-adds-rust: The Dev Container adds Rust to the template's Debian base image through a locked Feature, and installs the pinned tools on creation

- **Status:** 🟡 proposed
- **Date:** 2026-11-20
- **Deciders:** Markus Brigl
- **Applies to:** the `features` and the `postCreateCommand` of `.devcontainer/devcontainer.json`, and `.devcontainer/devcontainer-lock.json`
- **Extends:** NUC-container-hides-host-daemon §3

## Context

The template's Dev Container is `mcr.microsoft.com/devcontainers/base:debian` with the GitHub CLI
as a Feature, and carries no language toolchain. The toolchain of
[OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md) needs rustup, and installs
the toolchain pinned in `rust-toolchain.toml` and the further tools with
`scripts/install-tools.sh`. Nothing the specification's goals test needs a running service. The
decision was a clause of Forager's Dev Container ADR until the migration to NUC v0.9.0 moved the
template's part of it to [NUC-container-hides-host-daemon](NUC-container-hides-host-daemon.md).

## Decision

We will add Rust to the template's base image through a Feature:

1. **Feature.** The container adds the Feature `ghcr.io/devcontainers/features/rust:1` for rustup,
   pinned by its major version tag and locked in `devcontainer-lock.json` as
   [NUC-container-hides-host-daemon §3](NUC-container-hides-host-daemon.md) has every Feature.
2. **Tools.** The `postCreateCommand` runs `scripts/install-tools.sh`, which installs the toolchain
   that `rust-toolchain.toml` pins and the tools of
   [OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md).

**Out of scope:**

- the toolchain and its tools, which
  [OPS-pinned-stable-rust-toolchain](OPS-pinned-stable-rust-toolchain.md) decides;
- Node and its tools, and a browser, which the container does not carry;
- a container engine, which [OPS-container-reaches-no-engine](OPS-container-reaches-no-engine.md)
  decides.

## Alternatives considered

- **A custom `Dockerfile`** — full control of the image, but one more file to maintain and rebuild,
  for what a locked Feature and rustup already give.
- **The Rust Feature's `version` set to the pinned toolchain** — saves one toolchain download, at
  the cost of a second place that names the version; `none` would skip rustup altogether.
- **Compose with services** — nothing needs a service yet.

## Sources / Prior art

- The Rust Feature's definition and install script,
  <https://github.com/devcontainers/features/tree/main/src/rust> (version 1.6.0, checked
  2026-10-23): `version` defaults to `latest`; `none` installs no rustup.
- Dev Container specification — <https://containers.dev/implementors/json_reference/>.

## Consequences

- Positive: a new container builds the project without manual steps; the toolchain version lives
  in `rust-toolchain.toml` alone, used by the container, CI, and local runs alike.
- Negative / trade-offs: the Feature installs the latest stable beside the pinned toolchain, one
  unused toolchain in the image; `cargo install` compiles the tools on container creation, which
  takes minutes; parallel worktrees each compile the workspace into their own `target/`.
- Follow-ups: a shared build cache across worktrees, if build times hurt.

## Enforcement

[`scripts/check-devcontainer.sh`](../../scripts/check-devcontainer.sh) holds the Feature's tag and
its lock entry under [NUC-container-hides-host-daemon §3](NUC-container-hides-host-daemon.md).

**Not mechanically decidable:** that the container builds the project after creation, which the
review of a change to the Dev Container decides by rebuilding it.
