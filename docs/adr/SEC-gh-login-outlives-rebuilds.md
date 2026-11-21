# SEC-gh-login-outlives-rebuilds: The GitHub CLI's login lives in a host directory of the container's own, and a check of Forager's own holds its mount

- **Status:** 🟡 proposed
- **Date:** 2026-11-21
- **Deciders:** Markus Brigl
- **Applies to:** the `mounts`, `initializeCommand`, and `postCreateCommand` of `.devcontainer/devcontainer.json`, `scripts/check-gh-login.sh`, its self-test `scripts/test-check-gh-login.sh`, and its table `scripts/fixtures/check-gh-login.txt`
- **Supersedes:** [SEC-gh-login-stays-on-host](SEC-gh-login-stays-on-host.md)
- **Extends:** NUC-container-hides-host-daemon §2, NUC-main-only-via-pr §3

## Context

[SEC-gh-login-stays-on-host](SEC-gh-login-stays-on-host.md) keeps the GitHub CLI's login in the
host directory `~/.config/gh-devcontainer`, bound into the container. Its *Context*,
*Alternatives*, and *Sources* still hold. Its *Enforcement* names
`scripts/check-devcontainer.sh` and its self-test, both files the template ships.

Forager carried the rule of the mount inside those files, and its cases in the template's table
`scripts/fixtures/check-devcontainer.txt`. Each take-up of a release then had to merge the files
instead of replacing them. The take-up of NUC v0.11.1 merged 38 such files, ported each rule of
Forager's twice, and lost two of them from their tests until a review found it. The rule now stands
in a check of Forager's own, `scripts/check-gh-login.sh`, with its own self-test and table. The
template's files then carry no rule of Forager's.

This ADR restates every clause of SEC-gh-login-stays-on-host in its wording, but these:

- the title, which adds the check of Forager's own;
- the *Applies to*, which names the new check, its self-test, and its table instead of
  `scripts/check-devcontainer.sh`;
- the *Enforcement*, which names the new check and its job.

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

## Latitude

None.

## Alternatives considered

- **Keep the rule in `scripts/check-devcontainer.sh`** — the state before this ADR: every take-up
  of a release merges the template's check, its self-test, and its table by hand.
- **Send the rule to the template** — the mount is Forager's own, which the template's Dev
  Container does not carry; SEC-gh-login-stays-on-host leaves that open as a follow-up.
- The alternatives of SEC-gh-login-stays-on-host to the login itself still hold.

## Sources / Prior art

- The sources of [SEC-gh-login-stays-on-host](SEC-gh-login-stays-on-host.md).
- The take-up of NUC v0.11.1 in PR #222, its merges by hand and the first point of its review.

## Consequences

- Positive: a take-up replaces `scripts/check-devcontainer.sh`, its self-test, and its table with
  the release's; the rule keeps its own tests whatever the template changes.
- Negative / trade-offs: one more check and self-test of Forager's own to run; the check reads
  `devcontainer.json` as the template's does, without a JSON parser.
- Follow-ups: whether the template takes up the login's mount.

## Enforcement

`scripts/check-gh-login.sh` (job `devcontainer` in
[`checks.yml`](../../.github/workflows/checks.yml)) holds clause 1:

- It fails when `devcontainer.json` binds a host path ending in `.config/gh`.
- It fails when `devcontainer.json` binds `/home/vscode/.config/gh` from a host path that does not
  end in `.config/gh-devcontainer`.

Its self-test, `scripts/test-check-gh-login.sh`, reads its cases from
`scripts/fixtures/check-gh-login.txt` and cites this ADR.

**Not mechanically decidable:** that `initializeCommand` and `postCreateCommand` do what clauses 2
and 3 say, which the review of a change to the Dev Container decides.
