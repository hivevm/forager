# Every distribution of a Forager binary carries THIRD-PARTY-NOTICES.md

Observed: W. Randolph Franklin's pnpoly licence, whose code the label cache ports, asks in its
second condition that a redistribution in binary form reproduces his notice in its documentation
or other materials; the maintainer chose THIRD-PARTY-NOTICES.md for it. No binary is distributed
yet: the workspace is `publish = false`, there is no release workflow, scripts/build-web.sh writes
target/web/ (the wasm and its JavaScript glue) without the file, and `cargo build -p forager-cli`
gives a bare binary.

Before the first distribution of a binary, the CLI's archive and the browser bindings' package
carry THIRD-PARTY-NOTICES.md, LICENSE, and LICENSE-APACHE, and a check of the release fails,
naming the artefact, on one without the notices or whose copy lacks Franklin's copyright line. It
is a matter of licences and of how Forager is released, both the maintainer's (ADR-0006), and it
waits for the first release workflow.

Where: THIRD-PARTY-NOTICES.md, scripts/check-notice.sh, scripts/build-web.sh, Cargo.toml
(`publish`), .github/workflows/, Q-6 in docs/SPECIFICATION.md.
