# Ticket: Hold the frontend, the ports, and the adapters to the Q-2 ban

## What to build

Q-2 reads "no output path depends on hash-map iteration order, a clock, or a random source". The
ban of `clippy.toml` holds the crates of the IR and the core alone (#77, after the conformance
harness spec's "the core's freedom"), and its comment says hosts *and adapters* stay free. The
held set is the rows of the groups `Ir` and `Core` in the table of
`crates/arch-test/tests/dependencies.rs` (`ir_and_core`); `symbolize` has joined it since this
ticket was drafted, by its row. But the
order in which an adapter returns its features is the order the engine draws them, and the
frontend's style IR and diagnostics are output too: a `HashMap` in `adapter-shapefile`, in
`adapter-geojson`, or in `mapfile`'s translation would make the same input render or load
differently, and nothing would fail. None of those crates uses a hash map or a random source
today, but two unit tests of `mapfile` under `src/` bound their run time with
`std::time::Instant::now()` (`many_unterminated_bindings_lex_in_linear_time` in
`crates/mapfile/src/lexer.rs` and `includes_that_multiply_past_the_size_limit_are_an_error` in
`crates/mapfile/src/tests.rs`, both Q-9), so the opt-in line in `mapfile`'s `lib.rs` alone turns
clippy red. The session either lifts the ban for those two tests as `render-agg` lifts it for
its own, with an `#[expect]` whose reason names the time bound and an entry for each in
`LIFTED` of `crates/arch-test/tests/dependencies.rs`, or rewrites them to bound their work by a
count instead of a clock; the pull request says which and why.

The two `Verifies: Q-2` markers of the arch test and the one of `scripts/check-rust.sh` claim the
whole criterion, while the ban covers part of the output path. The bit-identity of G-9 holds since
f3a9c8f, whose comparison in `crates/cli/tests/msautotest.rs` claims Q-2 as well. By the
convention on `Verifies:` markers, a test that decides a part cites nothing. The session either
widens the ban so that the "no output path" clause is decided in full and says in the pull
request that the first clause rides on G-9, or the human decides the markers drop.

## Acceptance criteria

- [ ] Given `crates/mapfile`, `crates/ports`, and every `crates/adapter-*`, when the arch test
  runs, then each carries the opt-in line and lifts the ban only where `LIFTED` names it, as the
  IR and the core do; the two clocks of `mapfile`'s tests are either among those entries, each
  with its reason, or gone.
- [ ] Given the `clippy.toml` comment, the workspace lint comment in `Cargo.toml`, and the
  fixture step of `scripts/check-rust.sh`, when they are read, then they name the frontend, the
  ports, and the adapters as held, and only hosts and tooling as free.
- [ ] Given a `HashMap` added to an adapter's `src/`, when `scripts/check-rust.sh` runs, then
  clippy fails on it.
- [ ] Given `docs/ARCHITECTURE.md`, when the change lands, then the `arch-test` bullet no longer
  says that the frontend, the ports, and the adapters are not held to the ban, and names the
  hosts alone as free.
- [ ] Given the `Verifies: Q-2` markers, when the change lands, then the human has decided whether
  they stay, and the pull request says which.

## Read first

- Q-2 of `docs/SPECIFICATION.md`; [ADR-0007](../../adr/0007-rust-toolchain.md),
  [ADR-0008](../../adr/0008-ports-and-adapters-crates.md); the `Verifies:` entry of
  `docs/CONVENTIONS.md`.
- `clippy.toml`, `crates/arch-test/tests/dependencies.rs` (`TABLE`, `ir_and_core`, `LIFTED`),
  the lifted test of `crates/render-agg/src/lib.rs`, the Q-2 step of `scripts/check-rust.sh`;
  the PR body of #77.

## Blocked by

- None (can start immediately)

## Mode

with the human: it moves the reach of a quality goal's check, and the markers that claim Q-2 are
the human's call. The four tickets it waited for have landed: the one table of crate groups, the
one `poll_once` in `crates/ports/src/lib.rs`, the hosts' binding of sources through
`crates/engine/src/bind.rs`, and the properties a feature carries in the ports and both adapters.

## Maintainer's decision (2026-11-10)

The ban extends to the frontend, the ports, and the adapters. The two tests that bound their run
time with a clock are rewritten to bound their work by a count, so no exemption is needed.
