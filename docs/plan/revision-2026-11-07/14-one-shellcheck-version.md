# Ticket: Run one ShellCheck version in the Dev Container and in CI

## What to build

`scripts/check-shell.sh` runs whatever ShellCheck is on `PATH`: the Dev Container installs
Debian's package in its `postCreateCommand` (0.10.0 today), and CI takes the one the runner image
ships (`ubuntu-latest`, image `ubuntu-26.04`, version 20261101.186.1 on 2026-11-07), which is
newer. A newer ShellCheck brings new findings: #170 failed its `shell` job on SC2329, which
0.11 reports for a function called only indirectly where 0.10 reports SC2317, in
`scripts/test-merge-adr-index.sh`, after `check-all.sh` and the pre-push hook had passed locally.
The runner image moves without a change in the repository, so the gate can turn red on a branch
nobody touched, and a script that passes locally can fail in CI.

Pin one version and install it the same way in both places, as `scripts/install-tools.sh` does
for cargo-deny and the wasm-bindgen CLI (ADR-0008): for example ShellCheck's release binary of a
named version, checked by its published checksum, installed by `install-tools.sh` and by a step of
the `shell` job; or, at the least, have `check-shell.sh` print the version it runs and fail on one
other than the pinned. Where the binary comes from and how it is verified is a supply-chain choice
([ADR-0012](../../adr/0012-crate-supply-chain.md) covers crates only), and ADR-0002 names
ShellCheck as the one check dependency beyond the base image's tools; draft the decision with the
maintainer, a `proposed` ADR or a revision if it binds.

## Acceptance criteria

- [ ] Given the Dev Container after creation and the `shell` job of CI, when each runs
  `shellcheck --version`, then both report the pinned version.
- [ ] Given a ShellCheck of another version on `PATH`, when `scripts/check-shell.sh` runs, then it
  names both versions, and fails in CI.
- [ ] Given today's scripts, when the shell lint runs with the pinned version, then it passes.

## Read first

- [ADR-0002](../../adr/0002-decisions-verified-by-tests.md) (the checks and their dependencies),
  [ADR-0008](../../adr/0008-rust-toolchain.md) (`install-tools.sh`),
  [ADR-0010](../../adr/0010-rust-dev-container.md),
  [ADR-0011](../../adr/0011-action-references-major-tag.md); `scripts/check-shell.sh`,
  `scripts/install-tools.sh`, `.devcontainer/devcontainer.json`, the `shell` job of
  `.github/workflows/checks.yml`.

## Blocked by

- None (can start immediately)

## Mode

with the human: it adds a pinned tool and its source to the checks and the Dev Container, which
ADR-0002 and ADR-0010 decide, and the check script is the template's.

## Maintainer's decision (2026-11-07)

ShellCheck's release binary of a named version, verified by its published checksum, installed by
`scripts/install-tools.sh` and by a step of the `shell` job. The source of the tool is recorded
first in a `proposed` ADR.

On 2026-11-07 the maintainer chose ShellCheck 0.11.0, the findings of SC2329 fixed in the same
change; Debian's package leaves `postCreateCommand`; the ADR is a new one beside ADR-0002 and
ADR-0010, superseding neither.

## Maintainer's go-ahead (2026-11-08)

The agent drafts the ADR as Proposed; the maintainer sets it Accepted, and any ADR it supersedes to
Superseded.
