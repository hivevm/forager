# ADR-0044: The shell lint runs one ShellCheck, the release binary of a pinned version checked by its SHA-256

- **Status:** 🟢 accepted
- **Date:** 2026-11-10
- **Deciders:** Markus Brigl
- **Applies to:** `scripts/install-shellcheck.sh`, the ShellCheck of `scripts/check-shell.sh` and its self-test, the call in `scripts/install-tools.sh`, the `postCreateCommand` of `.devcontainer/devcontainer.json`, and the `shell` job of `.github/workflows/checks.yml`

## Context

[ADR-0002](0002-decisions-verified-by-tests.md) names ShellCheck as the one dependency of the
checks beyond bash and the base image's tools, and does not say where it comes from. The Dev
Container installed Debian's package in its `postCreateCommand` (0.10.0 on 2026-11-07), and CI
took the one the runner image ships, which was newer. A newer ShellCheck brings new findings: the
pull request #170 failed its `shell` job on SC2329, which 0.11 reports for a function called only
indirectly where 0.10 reports SC2317, after `check-all.sh` and the pre-push hook had passed
locally. The runner image moves without a change in the repository, so the gate could turn red on
a branch nobody touched.

[ADR-0008](0008-rust-toolchain.md) pins the tools `scripts/install-tools.sh` installs, and
[ADR-0012](0012-crate-supply-chain.md) decides where crates come from; neither covers a binary the
checks download. ShellCheck is written in Haskell, so `cargo install` does not build it, and
building it from source needs a Haskell toolchain the project has no other use for.

ShellCheck's maintainer publishes a statically linked binary per platform with each release on
GitHub (`koalaman/shellcheck`), and no checksum file beside it. GitHub records the SHA-256 of
every release asset when it is uploaded and shows it with the asset; for v0.11.0, released on
2025-08-04, `linux.x86_64.tar.xz` has
`8c3be12b05d5c177a04c29e3c78ce89ac86f1595681cab149b65b97c4e227198` and `linux.aarch64.tar.xz`
`12b331c1d2db6b9eb13cfca64306b1b157a86eb69db83023e261eaa7e7c14588`, read through GitHub's API on
2026-11-10, and the downloaded x86_64 archive hashes to its digest.

## Decision

We will run the shell lint, locally and in CI, with ShellCheck's release binary of the version
`scripts/install-shellcheck.sh` names, 0.11.0, downloaded from the `koalaman/shellcheck` release
on GitHub and refused unless its SHA-256 matches the digest pinned in that script, installed by
`scripts/install-tools.sh` in the Dev Container and by a step of the `shell` job in CI, and
`scripts/check-shell.sh` names the running and the pinned version where they differ, failing in a
CI run.

**Out of scope:** a platform other than Linux on x86_64 or aarch64, where the script fails and
names the platform; the other tools of the base image, which ADR-0002 leaves to it; when the pin
moves, which is a change of the script's version and digests, its findings fixed in the same
change.

## Alternatives considered

- **Debian's package in the Dev Container, the runner image's ShellCheck in CI** — the state
  before: two versions that differ and move on their own, the failure of #170.
- **Debian's package in both places** — the runner is Ubuntu, whose package lags Debian's and
  differs from it; one version in both places would still not hold.
- **`check-shell.sh` printing the version it runs and failing on another, installing nothing** —
  the least change, but it leaves the lint red until someone installs the right version by hand,
  in CI on every runner image update.
- **Building ShellCheck from source** — a Haskell toolchain in the container and in CI for one
  lint.
- **A container image or GitHub Action that ships ShellCheck** — a second supply chain to trust,
  and the Dev Container would still need its own install.

## Sources / Prior art

- ShellCheck releases, `https://github.com/koalaman/shellcheck/releases/tag/v0.11.0`, and the
  asset digests GitHub's REST API returns for it
  (`https://api.github.com/repos/koalaman/shellcheck/releases/tags/v0.11.0`), read 2026-11-10.
- ShellCheck's changelog for v0.11.0: SC2329, warn when (non-escaping) functions are never invoked.
- GitHub, *About releases*, on the digests of release assets,
  `https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases`.
- The install of cargo-deny and the wasm-bindgen CLI at pinned versions in
  `scripts/install-tools.sh` ([ADR-0008](0008-rust-toolchain.md)).

## Consequences

- Positive: the pre-push hook and CI lint with the same ShellCheck; a runner image update no longer
  changes the gate.
- Positive: the binary is checked against a digest in the repository, so a replaced asset fails
  the install.
- Negative / trade-offs: the digest is GitHub's, recorded at upload, not a signature of
  ShellCheck's maintainer; the trust is in the release as uploaded. The `shell` job and every
  container creation download about 2.5 MB.
- Negative / trade-offs: a container created before this change keeps Debian's ShellCheck in
  `/usr/bin`, ahead of `~/.local/bin` on `PATH`, until it is rebuilt; the install script says so,
  and the lint warns of the version.
- Follow-ups: moving the pin, its digests, and the findings a new version brings, together.

## Enforcement

`scripts/test-check-shell.sh` decides that a ShellCheck of another version than the pinned is
named with it and fails the lint in a CI run, and runs in the `shell` job before the lint, which
runs after the install step. `scripts/install-shellcheck.sh` fails on a digest that does not
match, through `sha256sum --check`.
