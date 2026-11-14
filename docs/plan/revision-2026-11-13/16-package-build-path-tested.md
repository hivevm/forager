# Ticket: Test the build path of scripts/package.sh

## What to build

`scripts/package.sh` (eaf1c52) builds the CLI's archive and the browser bindings' package, then
checks each. `scripts/test-package.sh` exercises only `--check` and `pack`. The build path, the
release build, `scripts/build-web.sh`, clearing `target/web/`, and `--target-dir`, runs nowhere:
not in `scripts/check-all.sh`, which runs it only on instruction, nor in CI. The fixes the review
asked for on that path were never run (eaf1c52's hand-off). `check_archives` also calls `exit` from
the sourced `scripts/lib/package.sh:45,48`, safe only because the self-test runs it in a child.
Run the whole flow in the self-test with `cargo`, `rustc`, and `scripts/build-web.sh` stubbed on
`PATH`, and let `check_archives` return its status.

## Acceptance criteria

- [ ] Given stubs that write a binary and a bundle, when `scripts/test-package.sh` runs, then
  `package.sh` without arguments writes both archives and the check passes over them.
- [ ] Given a stale file under the stubbed `target/web/`, when the package is built, then the
  archive does not hold it.
- [ ] Given `check_archives` sourced into a shell, when it finds a missing file, then it returns
  non-zero and the shell goes on.

## Read first

- Touches: `scripts/package.sh`, `scripts/lib/package.sh`, `scripts/test-package.sh`
- `docs/CONVENTIONS.md`, *A test that expects a refusal asserts its reason* and *A self-test
  asserts what it builds*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-test decides it; nothing is published; one revert. Stop when the flow cannot be
stubbed without changing what `package.sh` builds.
