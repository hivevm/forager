# Ticket: Fuzz the shapefile adapter weekly

## What to build

[ADR-0024](../../adr/0024-shapefile-reader.md) makes the shapefile reader Forager's own "to
maintain and to fuzz" and lists the adapter as a target of the weekly fuzz run among its
follow-ups; the PR body of #78 left it as doubt 5. Its tests hold a byte-mutation sweep over the
fixtures, which decides what those fixtures reach, not what libFuzzer finds. A fuzz target under
`fuzz/` that splits its input into a `.shp`, a `.shx`, and a `.dbf`, opens them through an
in-memory loader, and queries the source with an extent from the input, run by
`scripts/fuzz.sh` like the others, closes the follow-up. The corpus starts from the adapter's
test fixtures.

## Acceptance criteria

- [ ] Given `fuzz/Cargo.toml`, when it is read, then it has a `shapefile` target that depends on
  `forager-adapter-shapefile` and cites Q-3 as the other targets do.
- [ ] Given `scripts/fuzz.sh`, when the weekly run starts, then the new target runs for the same
  time as the others, with seeds under `fuzz/seeds/shapefile`. The script names its targets in a
  list of its own beside the `[[bin]]` entries of `fuzz/Cargo.toml`; a target added to one and not
  the other is never run, so the script reads the targets from the manifest or fails on a
  mismatch.
- [ ] Given a local run of the target for one minute, when it ends, then it reports no crash, and
  the pull request says how long it ran and with which seed corpus.
- [ ] Given `scripts/check-rust.sh`, when it runs, then the target is formatted and linted with the
  rest of the fuzz workspace.
- [ ] Given `docs/ARCHITECTURE.md`, when the target lands, then the fuzz paragraph names it.

## Read first

- [ADR-0024](../../adr/0024-shapefile-reader.md), Q-3 and Q-9 of `docs/SPECIFICATION.md`.
- `fuzz/fuzz_targets/geojson.rs` (the pattern to copy), `scripts/fuzz.sh`,
  `crates/adapter-shapefile/tests/common/mod.rs` (fixture builders).

## Blocked by

- None (can start immediately)

## Mode

unattended: the target is isolated in the fuzz workspace and a wrong result costs one revert; the
lint of the fuzz workspace and a one-minute local run decide it. Stop after two attempts at a
crash the target finds: file it as a ticket of its own with the input, rather than fixing the
reader here.
