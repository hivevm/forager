# Ticket: Fuzz the shapefile adapter weekly

## What to build

[ADR-0024](../../adr/0024-shapefile-reader.md) makes the shapefile reader Forager's own "to
maintain and to fuzz" and lists the adapter as a target of the weekly fuzz run among its
follow-ups; the PR body of #78 left it as doubt 5. Its tests hold a byte-mutation sweep over the
fixtures, which decides what those fixtures reach, not what libFuzzer finds. A fuzz target under
`fuzz/` that splits its input into a `.shp`, a `.shx`, and a `.dbf`, opens them through an
in-memory loader, and queries the source with an extent from the input, run by
`scripts/fuzz.sh` like the others, closes the follow-up. It polls the adapter's `open` and
`FeatureSource::features` with `forager_ports::poll_once`, which revision-2026-10-24 ticket 03
adds together with an arch test that refuses `Waker::noop` anywhere else, `fuzz/fuzz_targets`
included. The corpus starts from the adapter's test fixtures.

## Acceptance criteria

- [ ] Given `fuzz/Cargo.toml`, when it is read, then it has a `shapefile` target that depends on
  `forager-adapter-shapefile` and cites Q-3 as the other targets do.
- [ ] Given the target's source, when the arch test runs, then it finds no `Waker::noop` there:
  the target polls through `poll_once`.
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
- `fuzz/fuzz_targets/geojson.rs` (the pattern to copy), `poll_once` in
  `crates/ports/src/lib.rs`, `scripts/fuzz.sh`,
  `crates/adapter-shapefile/tests/common/mod.rs` (fixture builders).

## Blocked by

<!-- The target polls with the `poll_once` it adds, and its arch test would refuse a target that
     wrote its own no-op waker. -->

- docs/plan/revision-2026-10-24/03-one-poll-once.md

## Mode

unattended: the lint and format step of `scripts/check-rust.sh`, the arch test, the self-check
of `scripts/fuzz.sh` against the manifest, and a one-minute local run decide every criterion. The
target is isolated in the fuzz workspace and the script, and a wrong result costs one revert; it
touches nothing ADR-0007 keeps with the maintainer, the adapter's reader and its resource limits
included, which it only calls. It counts against the cap of three open agent branches of
[ADR-0007](../../adr/0007-harness.md), and is dispatched only while that cap allows. A wrong
outcome shows as a target the weekly run skips or that fails to build, which the manifest check
and the lint catch, or as a crash in the weekly run, which is filed against the reader. Stop
after two attempts at a crash the target finds: file it as a ticket of its own with the input,
rather than fixing the reader here.
