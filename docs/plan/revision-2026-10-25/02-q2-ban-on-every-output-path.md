# Ticket: Hold the frontend, the ports, and the adapters to the Q-2 ban

## What to build

Q-2 reads "no output path depends on hash-map iteration order, a clock, or a random source". The
ban of `clippy.toml` holds the crates of the IR and the core alone (#77, after the conformance
harness spec's "the core's freedom"), and its comment says hosts *and adapters* stay free. But the
order in which an adapter returns its features is the order the engine draws them, and the
frontend's style IR and diagnostics are output too: a `HashMap` in `adapter-shapefile`, in
`adapter-geojson`, or in `mapfile`'s translation would make the same input render or load
differently, and nothing would fail. None of those crates uses one today (the only `Instant`
outside the core is in tests), so widening the ban costs one line per crate.

The two `Verifies: Q-2` markers of the arch test and the one of `scripts/check-rust.sh` claim the
whole criterion, while the ban covers part of the output path and the bit-identity of G-9 is
still pending (conformance-harness ticket 10). By the convention on `Verifies:` markers, a test
that decides a part cites nothing. The session either widens the ban so that the "no output path"
clause is decided in full and says in the pull request that the first clause rides on G-9, or
the human decides the markers drop until G-9 holds.

## Acceptance criteria

- [ ] Given `crates/mapfile`, `crates/ports`, and every `crates/adapter-*`, when the arch test
  runs, then each carries the opt-in line and lifts the ban nowhere, as the IR and the core do.
- [ ] Given the `clippy.toml` comment, the workspace lint comment in `Cargo.toml`, and the
  fixture step of `scripts/check-rust.sh`, when they are read, then they name the frontend, the
  ports, and the adapters as held, and only hosts and tooling as free.
- [ ] Given a `HashMap` added to an adapter's `src/`, when `scripts/check-rust.sh` runs, then
  clippy fails on it.
- [ ] Given the `Verifies: Q-2` markers, when the change lands, then the human has decided whether
  they stay, and the pull request says which.

## Read first

- Q-2 of `docs/SPECIFICATION.md`; [ADR-0008](../../adr/0008-rust-toolchain.md),
  [ADR-0009](../../adr/0009-ports-and-adapters-crates.md); the `Verifies:` entry of
  `docs/CONVENTIONS.md`.
- `clippy.toml`, `crates/arch-test/tests/dependencies.rs`, the Q-2 step of
  `scripts/check-rust.sh`; the PR body of #77.

## Blocked by

- docs/plan/revision-2026-10-25/01-one-table-of-crate-groups.md
- docs/plan/revision-2026-10-24/03-one-poll-once.md
- docs/plan/revision-2026-10-24/04-hosts-bind-sources-alike.md

## Mode

with the human: it moves the reach of a quality goal's check, and the markers that claim Q-2 are
the human's call. The two revision-2026-10-24 tickets block it because they are editing
`crates/ports`.
