# ADR-0009: The Dev Container keeps the host daemon out of reach, its Features locked, and each project under a path of its own, adds Rust to the Debian base image, and keeps the GitHub CLI's login on the host

- **Status:** 🟢 accepted
- **Date:** 2026-11-10
- **Deciders:** NUC maintainer, Markus Brigl
- **Applies to:** `.devcontainer/` (its Features, lock, mounts, `initializeCommand`, `postCreateCommand`, and `postStartCommand` among it), `.vscode/settings.json`, `scripts/check-devcontainer.sh`, and everything the container pulls in

## Context

The template ships a Dev Container so that the first run is reproducible without committing to a
language toolchain: `mcr.microsoft.com/devcontainers/base:debian` with the GitHub CLI as a
Feature. Agents often want Docker access, and the obvious way — the `docker-outside-of-docker`
Feature — bind-mounts the **host** Docker socket, handing host-level control to anything inside.
Managing host containers from VS Code does not need the socket.

The container also pulls in third-party code that runs next to the agent credentials it mounts:
**Features**, executed at build time. A Feature tag is mutable and can be repointed, so a tag alone
does not say what a commit builds.

A third cost comes from the mount. An agent that remembers across sessions keys what it
remembers by the directory it works in, and every session loads that memory before its first
step. A container that mounts every project at the same path gives every project one memory.

The toolchain of [ADR-0007](0007-rust-toolchain.md) needs rustup, and installs the toolchain
pinned in `rust-toolchain.toml` and the further tools with `scripts/install-tools.sh`. Nothing the
specification's goals test needs a running service or a container. The harness
([ADR-0006](0006-harness.md)) keeps parallel sessions apart as worktrees in one repository.

The GitHub CLI (`gh`) keeps its login in `~/.config/gh/hosts.yml` inside the container, where no
keyring is. A rebuild throws it away, and every new `gh auth login` mints a new OAuth token for
the GitHub CLI's OAuth app. GitHub keeps only a limited number of tokens per user, app, and scope,
and revokes the oldest beyond it. On 2026-11-07 the login was lost or turned invalid four times in
one session, twice after the pre-push hook had run its checks for some fifteen minutes, so that
the push failed at its end with HTTP 401. Git meanwhile still asked the credential helper VS Code
had installed for an earlier container, whose script no longer existed.
[`AGENTS.md` §6](../../AGENTS.md#6-project-rules) has `gh` authenticated through its web flow, a
human entering the one-time code, and never a personal access token requested, stored, or
hard-coded. The container already binds the host's `~/.claude` for the agent's configuration.

## Decision

We will keep the host's container engine and every other engine out of the Dev Container's reach,
lock its Features, mount each project under a path of its own, add Rust, and keep `gh`'s login on
the host:

- **Host engine.** No variant may mount the host Docker socket, nor add a Feature that would mount
  one. Host containers are managed from an extension pinned to the host side via
  `remote.extensionKind` in [`.vscode/settings.json`](../../.vscode/settings.json).
- **No engine.** The container must give no access to any container engine.
- **Features.** Every Feature is pinned by its major version tag. The repository commits
  `.devcontainer/devcontainer-lock.json`, so the same commit yields the same Features.
- **Mount path.** The repository is mounted at `/workspaces/` and the name of its directory on the
  host, not at a path every project shares.
- **Rust.** The container adds the Feature `ghcr.io/devcontainers/features/rust:1`, locked in
  `devcontainer-lock.json`, to the template's base image for rustup. Its `postCreateCommand` runs
  `scripts/install-tools.sh` for the pinned toolchain and tools.
- **GitHub CLI login.** One login through the web flow must outlive every rebuild:
  - the container binds the host directory `~/.config/gh-devcontainer` to
    `/home/vscode/.config/gh`;
  - `initializeCommand` creates that directory on the host;
  - `postCreateCommand` runs `gh auth setup-git`, which points git at `gh`'s stored login.

**Out of scope:**

- whether the container pins extensions;
- whether work may happen outside the container;
- hardening the container, which is not a security boundary;
- how the lock is refreshed;
- a container engine for an adapter whose tests need a database, decided with that adapter;
- a container per session;
- Node and its tools;
- the host's own `~/.config/gh`, which on macOS and Windows holds no token, `gh` keeping it in the
  system keyring;
- a token passed by environment (`GH_TOKEN`), which the rules forbid;
- the other credential helpers VS Code injects, which `gh auth setup-git` overrides for
  `github.com` alone;
- which scopes the login asks for.

## Alternatives considered

- **`docker-outside-of-docker`** — mounts the host socket and hands host-level control to
  everything inside, the agent included.
- **Ignore the Feature lock** — a repointed tag reaches the credentials before anyone sees a diff.
- **Digests directly in `devcontainer.json`** — buries hashes in the hand-edited file and loses the
  readable tag.
- **One fixed mount path, such as `/workspace`** — shorter to type, and every project opened in
  the container shares the memory an agent keeps under it, so each session pays for all of them.
- **A custom `Dockerfile`** — full control of the image, but one more file to maintain and
  rebuild, for what a locked Feature and rustup already give.
- **The Rust Feature's `version` set to the pinned toolchain** — saves one toolchain download, at
  the cost of a second place that names the version; `none` would skip rustup altogether.
- **Compose with services** — nothing needs a service yet.
- **Log in to `gh` after every rebuild** — the state before 2026-11-07: a new token each time, the
  oldest revoked, and pushes failing at their end.
- **Bind the host's `~/.config/gh`** — one login for host and container, but empty in the
  container wherever the host keeps the token in its keyring, and the container could then
  rewrite the host's own configuration.
- **`GH_TOKEN` from the host's environment (`localEnv`)** — no file, but a token the human
  creates and stores, against [`AGENTS.md` §6](../../AGENTS.md#6-project-rules).
- **A named Docker volume for the login** — survives a rebuild without a host path, but is lost
  with the volume, invisible to the human, and a second store to clean up.

## Sources / Prior art

- Dev Container specification, including `mounts` and `initializeCommand` —
  <https://containers.dev/implementors/json_reference/> and <https://containers.dev/features>.
- `remote.extensionKind` — <https://code.visualstudio.com/docs/devcontainers/containers>.
- Docker daemon attack surface —
  <https://docs.docker.com/engine/security/#docker-daemon-attack-surface>.
- The Rust Feature's definition and install script,
  <https://github.com/devcontainers/features/tree/main/src/rust> (version 1.6.0, checked
  2026-10-23): `version` defaults to `latest`; `none` installs no rustup.
- The catalogue entry *Container access* of the `propose-adr` skill.
- GitHub CLI, `gh auth login` (credentials in the system keyring where one is, else in
  `hosts.yml`) and `gh auth setup-git`: <https://cli.github.com/manual/gh_auth_login>,
  <https://cli.github.com/manual/gh_auth_setup-git>.
- GitHub, limits on OAuth tokens per user, app, and scope:
  <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#creating-multiple-tokens-for-oauth-apps>.

## Consequences

- Positive: the security posture is the same in every variant; the same commit gives the same
  Features, and a changed digest shows up in review; an agent's memory stays with the project
  it was written in.
- Positive: a new container builds the project without manual steps; the toolchain version lives
  in `rust-toolchain.toml` alone, used by the container, CI, and local runs alike.
- Positive: one `gh` login per host, outliving rebuilds; pushes no longer fail at their end on a
  token revoked meanwhile; git and `gh` use the same login.
- Negative / trade-offs: Feature updates stop arriving on their own and a stale lock is invisible.
  A project that moves off a shared mount path leaves the memory filed under the old one behind,
  and the linked worktrees it made there name the old path until `git worktree repair` runs. Two
  projects whose directories on the host share a name still share one memory.
- Negative / trade-offs: the Rust Feature installs the latest stable beside the pinned toolchain,
  one unused toolchain in the image; `cargo install` compiles the tools on container creation,
  which takes minutes; parallel worktrees each compile the workspace into their own `target/`.
- Negative / trade-offs: the OAuth token lies in a file on the host, in a directory the human
  does not otherwise use, readable by anything that runs as the user there and in the container;
  logging out (`gh auth logout`) or deleting the directory removes it. The login's mount is this
  project's own, which the template's Dev Container does not carry.
- Follow-ups: whether an automated update path for Features is wired up; a shared build cache
  across worktrees, if build times hurt; whether the template takes up the login's mount.

## Enforcement

[`scripts/check-devcontainer.sh`](../../scripts/check-devcontainer.sh) (job `devcontainer` in
[`checks.yml`](../../.github/workflows/checks.yml)) reads the files this decision names and fails
when:

1. [`devcontainer.json`](../../.devcontainer/devcontainer.json) mounts a host socket or adds a
   Feature that would;
2. a Feature carries no major version tag;
3. the lock is missing or names other Features than the definition;
4. [`.vscode/settings.json`](../../.vscode/settings.json) no longer pins the container-management
   extension to the host side;
5. the container's `postStartCommand` no longer enables the repository's git hooks with the merge
   settings they rely on (`core.hooksPath`, `merge.ff`, `pull.ff`, `merge.adr-index.driver`), the
   container's share of the local gate of [ADR-0002](0002-decisions-verified-by-tests.md);
6. `workspaceFolder` or the target of `workspaceMount` is not
   `/workspaces/${localWorkspaceFolderBasename}`;
7. `devcontainer.json` binds a host path ending in `.config/gh`, the host's own configuration, or
   binds the container's `/home/vscode/.config/gh` from anything but a source ending in
   `.config/gh-devcontainer`.

Its self-test, `scripts/test-check-devcontainer.sh`, cites this ADR with `Verifies:`
([ADR-0002](0002-decisions-verified-by-tests.md)).

**Not mechanically decidable:** the check reads neither of these, and a reviewer holds them:

- a variant definition under a subdirectory of `.devcontainer/`, which the check does not read;
- whether the lock's digests are the ones the tags currently resolve to;
- the mount of a definition built from a compose file, which that file declares;
- the container itself, which no CI job builds: that it builds the project, that the login's
  directory exists on the host, and that `gh auth setup-git` ran;
- a Feature that would run an engine inside the container, which review catches.
