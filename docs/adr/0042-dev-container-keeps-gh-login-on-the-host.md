# ADR-0042: The Dev Container keeps the GitHub CLI's login in a directory of its own on the host

- **Status:** 🟡 proposed
- **Date:** 2026-11-07
- **Deciders:** Markus Brigl
- **Applies to:** the `mounts`, `initializeCommand`, and `postCreateCommand` of `.devcontainer/devcontainer.json` that concern the GitHub CLI, and `scripts/check-devcontainer.sh`

## Context

The GitHub CLI (`gh`), which the Dev Container installs as a Feature, keeps its login in
`~/.config/gh/hosts.yml` inside the container, where no keyring is. A rebuild of the container
throws it away, and every new `gh auth login` mints a new OAuth token for the GitHub CLI's OAuth
app; GitHub keeps only a limited number of tokens per user, app, and scope and revokes the oldest
beyond it. On 2026-11-07 the login was lost or turned invalid four times in one session, twice
after the pre-push hook had run its checks for some fifteen minutes, so that the push failed at
its end with HTTP 401. Git meanwhile still asked the credential helper VS Code had installed for
an earlier container, whose script no longer existed.

[`AGENTS.md` §6](../../AGENTS.md#6-project-rules) has `gh` authenticated through its web flow, a
human entering the one-time code, and never a personal access token requested, stored, or
hard-coded. [ADR-0004](0004-dev-container-runtime.md) keeps the host's container engine out of
reach and leaves hardening the container, which is not a security boundary, out of scope; the
container already binds the host's `~/.claude` for the agent's configuration.

## Decision

We will bind a directory of the host's own for the GitHub CLI, `~/.config/gh-devcontainer`, to
`/home/vscode/.config/gh` in the Dev Container, created on the host by `initializeCommand`, and
point git at `gh`'s stored login by `gh auth setup-git` in `postCreateCommand`, so that one login
through the web flow outlives every rebuild.

**Out of scope:** the host's own `~/.config/gh`, which on macOS and Windows holds no token, `gh`
keeping it in the system keyring; a token passed by environment (`GH_TOKEN`), which the rules
forbid; the other credential helpers VS Code injects, which `gh auth setup-git` overrides for
`github.com` alone; which scopes the login asks for.

## Alternatives considered

- **Log in after every rebuild, as today** — no change, but a new token each time, the oldest
  revoked, and pushes failing at their end.
- **Bind the host's `~/.config/gh`** — one login for host and container, but empty in the
  container wherever the host keeps the token in its keyring, and the container could then
  rewrite the host's own configuration.
- **`GH_TOKEN` from the host's environment (`localEnv`)** — no file, but a token the human
  creates and stores, against [`AGENTS.md` §6](../../AGENTS.md#6-project-rules).
- **A named Docker volume** — survives a rebuild without a host path, but is lost with the
  volume, invisible to the human, and a second store to clean up.

## Sources / Prior art

- Dev Containers, `mounts` and `initializeCommand`:
  <https://containers.dev/implementors/json_reference/>, the pattern the `~/.claude` mount of this
  file already follows.
- GitHub CLI, `gh auth login` (credentials in the system keyring where one is, else in
  `hosts.yml`) and `gh auth setup-git`: <https://cli.github.com/manual/gh_auth_login>,
  <https://cli.github.com/manual/gh_auth_setup-git>.
- GitHub, limits on OAuth tokens per user, app, and scope:
  <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#creating-multiple-tokens-for-oauth-apps>.

## Consequences

- Positive: one login per host, outliving rebuilds; pushes no longer fail at their end on a
  token revoked meanwhile; git and `gh` use the same login.
- Negative / trade-offs: the OAuth token lies in a file on the host, in a directory the human
  does not otherwise use, readable by anything that runs as the user there and in the container;
  logging out (`gh auth logout`) or deleting the directory removes it.
- Negative / trade-offs: the file belongs to the template's Dev Container; the change is to be
  raised with the template as well, or carried as this project's own.
- Follow-ups: whether the template takes it up.

## Enforcement

`scripts/check-devcontainer.sh` fails when `.devcontainer/devcontainer.json` binds a host path
ending in `.config/gh` (the host's own configuration) to the container, and when the container's
`/home/vscode/.config/gh` is bound from anything but a source ending in
`.config/gh-devcontainer`; its self-test cites this ADR with `Verifies: ADR-0042`. Not checked:
that the directory exists on the host, and that `gh auth setup-git` ran — no CI job builds the
container.
