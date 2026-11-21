# SEC-gh-login-stays-on-host: The GitHub CLI's login lives in a host directory of the container's own and outlives every rebuild

- **Status:** ⚪ superseded by SEC-gh-login-outlives-rebuilds
- **Date:** 2026-11-20
- **Deciders:** Markus Brigl
- **Applies to:** the `mounts`, `initializeCommand`, and `postCreateCommand` of `.devcontainer/devcontainer.json`, and `scripts/check-devcontainer.sh`
- **Extends:** NUC-container-hides-host-daemon §2, NUC-main-only-via-pr §3

## Context

The GitHub CLI (`gh`) keeps its login in `~/.config/gh/hosts.yml` inside the container, where no
keyring is. A rebuild throws it away, and every new `gh auth login` mints a new OAuth token for the
GitHub CLI's OAuth app. GitHub keeps only a limited number of tokens per user, app, and scope, and
revokes the oldest beyond it. On 2026-11-07 the login was lost or turned invalid four times in one
session, twice after the pre-push hook had run its checks for some fifteen minutes, so that the
push failed at its end with HTTP 401. Git meanwhile still asked the credential helper VS Code had
installed for an earlier container, whose script no longer existed.
[NUC-main-only-via-pr §3](NUC-main-only-via-pr.md) has `gh` signed in through its web flow, a human
entering the one-time code, and never a personal access token requested, stored, or hard-coded.
The container already binds the host's `~/.claude` for the agent's configuration. The decision was
a clause of Forager's Dev Container ADR until the migration to NUC v0.9.0.

## Decision

We will keep one login through the web flow on the host, for the container alone:

1. **Mount.** The container binds the host directory `~/.config/gh-devcontainer` to
   `/home/vscode/.config/gh`, and never the host's own `~/.config/gh`.
2. **Directory.** `initializeCommand` creates that directory on the host.
3. **Git.** `postCreateCommand` runs `gh auth setup-git`, which points git at `gh`'s stored login.

**Out of scope:**

- the host's own `~/.config/gh`, which on macOS and Windows holds no token, `gh` keeping it in the
  system keyring;
- a token passed by environment (`GH_TOKEN`), which the rules forbid;
- the other credential helpers VS Code injects, which `gh auth setup-git` overrides for
  `github.com` alone;
- which scopes the login asks for.

## Alternatives considered

- **Log in to `gh` after every rebuild** — the state before 2026-11-07: a new token each time, the
  oldest revoked, and pushes failing at their end.
- **Bind the host's `~/.config/gh`** — one login for host and container, but empty in the container
  wherever the host keeps the token in its keyring, and the container could then rewrite the host's
  own configuration.
- **`GH_TOKEN` from the host's environment (`localEnv`)** — no file, but a token the human creates
  and stores, against [NUC-main-only-via-pr §3](NUC-main-only-via-pr.md).
- **A named Docker volume for the login** — survives a rebuild without a host path, but is lost
  with the volume, invisible to the human, and a second store to clean up.

## Sources / Prior art

- Dev Container specification, `mounts` and `initializeCommand` —
  <https://containers.dev/implementors/json_reference/>.
- GitHub CLI, `gh auth login` (credentials in the system keyring where one is, else in
  `hosts.yml`) and `gh auth setup-git`: <https://cli.github.com/manual/gh_auth_login>,
  <https://cli.github.com/manual/gh_auth_setup-git>.
- GitHub, limits on OAuth tokens per user, app, and scope:
  <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#creating-multiple-tokens-for-oauth-apps>.

## Consequences

- Positive: one `gh` login per host, outliving rebuilds; pushes no longer fail at their end on a
  token revoked meanwhile; git and `gh` use the same login.
- Negative / trade-offs: the OAuth token lies in a file on the host, in a directory the human does
  not otherwise use, readable by anything that runs as the user there and in the container;
  logging out (`gh auth logout`) or deleting the directory removes it. The mount is Forager's own,
  which the template's Dev Container does not carry.
- Follow-ups: whether the template takes up the login's mount.

## Enforcement

[`scripts/check-devcontainer.sh`](../../scripts/check-devcontainer.sh) (job `devcontainer` in
[`checks.yml`](../../.github/workflows/checks.yml)) fails when `devcontainer.json` binds a host
path ending in `.config/gh`, or binds `/home/vscode/.config/gh` from a host path that does not end
in `.config/gh-devcontainer`. Its self-test, `scripts/test-check-devcontainer.sh`, cites this ADR.

**Not mechanically decidable:** that `initializeCommand` and `postCreateCommand` do what clauses 2
and 3 say, which the review of a change to the Dev Container decides.
