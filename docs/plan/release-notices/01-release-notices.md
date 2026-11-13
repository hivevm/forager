# Ticket: Every distribution of a Forager binary carries THIRD-PARTY-NOTICES.md

## What to build

W. Randolph Franklin's pnpoly licence, whose code the label cache ports, asks in its second
condition that a redistribution in binary form reproduces his notice in its documentation or other
materials; the maintainer chose `THIRD-PARTY-NOTICES.md` for it. The file holds MapServer's notice
beside his and says that every distribution of a binary carries it. No binary is distributed yet:
the workspace's `Cargo.toml` sets `publish = false`, which every crate inherits; no workflow under
`.github/workflows/` builds a release (the `v0.x` tags are releases of the template, not of
Forager); `scripts/build-web.sh` writes `target/web/` (the wasm and its JavaScript glue) without
the file; and `cargo build -p forager-cli` gives a bare binary, `forager`. `scripts/check-notice.sh`
holds the file in the tree to Franklin's and MapServer's lines, but nothing holds an artefact to
it.

Before the first distribution of a binary, the CLI's archive and the browser bindings' package
carry `THIRD-PARTY-NOTICES.md`, `LICENSE`, and `LICENSE-APACHE`, and a check of the release holds
each artefact to it.

## Acceptance criteria

- [ ] Given the artefacts a release builds, the CLI's archive and the browser bindings' package,
  when they are listed, then each holds `THIRD-PARTY-NOTICES.md`, `LICENSE`, and `LICENSE-APACHE`.
- [ ] Given an artefact without `THIRD-PARTY-NOTICES.md`, or one whose copy lacks Franklin's or
  MapServer's copyright line, when the release's check runs, then it fails naming the artefact.

## Read first

- Touches: the release workflow to come under `.github/workflows/`, `scripts/build-web.sh`,
  `Cargo.toml` (`publish`), `THIRD-PARTY-NOTICES.md`
- `scripts/check-notice.sh`, whose lines for Franklin's and MapServer's notices the release's
  check reuses
- Q-6 and the licence line of *Constraints* in `docs/SPECIFICATION.md`

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a matter of licences and of how Forager is released, both the maintainer's
([ADR-0006](../../adr/0006-harness.md)); it waits for the first release workflow.
