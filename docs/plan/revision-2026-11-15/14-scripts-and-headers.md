# Ticket: The scripts' copies and stale headers of this scope

## What to build

- `scripts/test-sensor-duplication.sh:97-113` writes its own `expect`, though
  `scripts/lib/expect.sh` (36bef9a8) was written for the project's own self-tests. Nine of them
  source it, and `test-sensor-backlog-commits.sh:27` wraps it as `expect_sensor`. The local copy
  accepts several needles. Wrap the shared one the same way.
- `scripts/install-jscpd.sh:15-50` copies the structure of `scripts/install-shellcheck.sh:15-50`:
  the case of the architecture, the probe of the version, the download, the checksum, and the
  install. The pins, assets, and messages differ. Its header (lines 6-7) says only the sensors job
  runs it. The docs and shell jobs of `.github/workflows/checks.yml` install it too. Correct the
  header. Share the installer through `scripts/lib/` unless `install-shellcheck.sh` is the
  template's (ADR-0004), in which case say so in the header.
- The header of `.github/workflows/noise.yml:3-9` names G-6 and G-13 only, though the workflow also
  writes G-11's records (ADR-0038) and its paths include `crates/proj/tests/reference/**`.
- The header of `crates/cli/tests/conformance/mapserver_proj.c:2-8` speaks of "the list of points
  named as its one argument" before it describes the rectangles and the shapes. Its line 24 runs
  past the width of the rest. The doc comment of the lists in
  `crates/cli/tests/conformance/noise/mod.rs:126-127` names points and rectangles, but the lists are
  three, shapes included.
- ADR-0037's *Enforcement* names the arch test that holds the three crate copies of `clippy.toml`
  equal to the workspace's. That test,
  `the_crates_that_port_mapservers_drawing_ban_what_the_workspace_bans` in
  `crates/arch-test/tests/dependencies.rs`, cites `Q-2, ADR-0007` only. Add `ADR-0037` to its marker
  if it decides that clause, as the convention on `Verifies:` markers asks.

## Acceptance criteria

- [ ] Given `scripts/test-sensor-duplication.sh`, when it runs, then it passes, with the same
  cases and counts, through `scripts/lib/expect.sh`.
- [ ] Given the headers named above, when the change lands, then each says what its file does.
- [ ] Given `scripts/check-traceability.sh`, when it runs, then it passes.

## Read first

- Touches: `scripts/test-sensor-duplication.sh`, `scripts/install-jscpd.sh`,
  `scripts/install-shellcheck.sh`, `scripts/lib/`, `.github/workflows/noise.yml`,
  `crates/cli/tests/conformance/mapserver_proj.c`, `crates/arch-test/tests/dependencies.rs`
- `docs/adr/0007-rust-toolchain.md`, `docs/adr/0004-template-releases.md`,
  `docs/adr/0038-proj-references-from-mapserver.md`; `docs/CONVENTIONS.md`, *A self-test asserts
  what it builds*.

## Blocked by

- None (can start immediately)

## Mode

unattended: the self-tests, `scripts/check-shell.sh`, and the traceability check decide it; one
revert. Stop when a file is the template's, or when a self-test's case or count would change. It
touches nothing ADR-0006 keeps with the human, and it counts against ADR-0006's cap of three open
agent branches.
