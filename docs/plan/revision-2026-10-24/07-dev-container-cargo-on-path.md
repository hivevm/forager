# Ticket: Rebuild the Dev Container with the Rust Feature and confirm the hooks find cargo

## What to build

The Dev Container that did the browser tracer's work was never rebuilt with the Rust Feature
of [ADR-0010](../../adr/0010-rust-dev-container.md) (#11, *Could not confirm*). Its toolchain
sits in `~/.cargo/bin`, which is not on the `PATH` of the shell the git hooks run in (`PATH` there
is `/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin:/home/vscode/.local/bin`), so the
pre-push hook goes red until a session exports it by hand. The refusal then reads
"install-tools: rustup is required (https://rustup.rs)" (`scripts/install-tools.sh:14-16`),
which sends the reader to install what is installed.

The maintainer rebuilds the container from `.devcontainer/devcontainer.json`, and a session
confirms that the hooks find the toolchain without help. `install-tools.sh` names the likelier
cause when rustup is missing from `PATH` but present in `${CARGO_HOME:-$HOME/.cargo}/bin`.

## Acceptance criteria

- [ ] Given the rebuilt container, when `bash -c 'command -v cargo rustup'` runs in a fresh
  non-interactive shell, then both are found, and `env -i HOME="$HOME" PATH=<the container's
  default> bash .githooks/pre-push` in the main checkout runs to green.
- [ ] Given rustup in `$HOME/.cargo/bin` and not on `PATH`, when `scripts/install-tools.sh`
  runs, then it fails with a message naming that directory and `PATH`.

## Read first

- [ADR-0010](../../adr/0010-rust-dev-container.md),
  [ADR-0004](../../adr/0004-dev-container-runtime.md),
  [ADR-0008](../../adr/0008-rust-toolchain.md); `.githooks/pre-push`, `scripts/install-tools.sh`.

## Blocked by

- None (can start immediately)

## Mode

with the human: only the maintainer rebuilds the container, and the confirmation runs in it.
