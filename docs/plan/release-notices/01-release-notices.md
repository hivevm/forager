# Ticket: Every distribution of a Forager binary carries THIRD-PARTY-NOTICES.md

## What to build

W. Randolph Franklin's pnpoly licence, whose code the label cache ports, asks in its second
condition that a redistribution in binary form reproduces his notice in its documentation or other
materials; the maintainer chose `THIRD-PARTY-NOTICES.md` for it. No binary is distributed yet:
every crate is `publish = false`, there is no release workflow, `scripts/build-web.sh` writes
`target/web/` (the wasm and its JavaScript glue) without the file, and `cargo build -p forager-cli`
gives a bare binary. Before the first distribution of a binary, the CLI's archive and the browser
bindings' package carry `THIRD-PARTY-NOTICES.md`, `LICENSE`, and `LICENSE-APACHE`, and a check
holds each artefact to it.

## Acceptance criteria

- [ ] Given the artefacts a release builds, the CLI's archive and the browser bindings' package,
  when they are listed, then each holds `THIRD-PARTY-NOTICES.md`, `LICENSE`, and `LICENSE-APACHE`.
- [ ] Given an artefact without `THIRD-PARTY-NOTICES.md`, or one whose copy lacks Franklin's
  copyright line, when the release's check runs, then it fails naming the artefact.

## Read first

- `THIRD-PARTY-NOTICES.md`; `scripts/check-notice.sh`; `scripts/build-web.sh`; Q-6 in
  `docs/SPECIFICATION.md`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it is a matter of licences and of how Forager is released, both the maintainer's
([ADR-0007](../../adr/0007-harness.md)); it waits for the first release workflow.
